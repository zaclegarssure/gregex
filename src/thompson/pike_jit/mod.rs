use std::alloc::{self, Layout};
use std::error::Error;
use std::fmt::Display;
use std::{fmt, mem};

use cg_impl_array::CGImplArray;
use cg_impl_cow_array::CGImplCowArray;
use cg_impl_register::CGImplReg;
use cg_implementation::CGImpl;
use dynasmrt::{
    AssemblyOffset, DynamicLabel, DynasmApi, DynasmLabelApi, ExecutableBuffer, dynasm,
    x64::Assembler,
};
use regex_syntax::Parser;
use regex_syntax::hir::Look;

use crate::regex::{Config, RegexImpl};
use crate::thompson::bytecode::Instruction;
use crate::util::{Char, Input, Span, find_prev_char};

use super::bytecode::{Bytecode, Compiler};

/// Defines the platform and register aliases
macro_rules! __ {
    ($ops: expr, $($t:tt)*) => {
        dynasm!($ops
        ; .arch x64
        ; .alias retval, rax
        ; .alias curr_thd_data, rax
        ; .alias span_end, rbx
        ; .alias mem, r8
        ; .alias curr_char, ecx
        ; .alias input_pos, rdx
        ; .alias input, rdi
        ; .alias input_len, rsi
        ; .alias next_tail, r9
        ; .alias curr_top, r10
        ; .alias cg_reg, r11
        ; .alias prev_char, r12d
        ; .alias reg1, r13
        ; .alias reg1d, r13d
        ; .alias reg2, r14
        ; .alias reg2d, r14d
        ; .alias input_inc, r15
        ; $($t)*
        )
    };
}

/// Slighly hacky way to have something similar to constexpr in C++ to declare
/// constants with values depending on other constants.
macro_rules! cst {
    ($name: ident, $value: expr) => {
        macro_rules! $name {
            () => {
                $value
            };
        }
    };
}

// Stack-layout
cst!(ptr_size, 8);
cst!(frame_ptr_offset, 0);
cst!(return_addr_offset, frame_ptr_offset!() + ptr_size!());
cst!(span_end_offset, return_addr_offset!() + ptr_size!());
cst!(return_on_accept, span_end_offset!() + ptr_size!());
cst!(prev_char_offset, return_on_accept!() + ptr_size!());
cst!(result_offset, frame_ptr_offset!() - ptr_size!());
cst!(result_len_offset, result_offset!() - ptr_size!());
cst!(saved_rbx_offset, result_len_offset!() - ptr_size!());
cst!(saved_r12_offset, saved_rbx_offset!() - ptr_size!());
cst!(saved_r13_offset, saved_r12_offset!() - ptr_size!());
cst!(saved_r14_offset, saved_r13_offset!() - ptr_size!());
cst!(saved_r15_offset, saved_r14_offset!() - ptr_size!());
cst!(next_tail_init_offset, saved_r15_offset!() - ptr_size!());
cst!(curr_top_init_offset, next_tail_init_offset!() - ptr_size!());
cst!(state_ptr_offset, curr_top_init_offset!() - ptr_size!());
cst!(last_saved_value_offset, state_ptr_offset!());

pub mod cg_impl_array;
pub mod cg_impl_cow_array;
pub mod cg_impl_register;
pub mod cg_impl_tree;
pub mod cg_implementation;

#[derive(Debug)]
pub struct JittedRegex {
    code: ExecutableBuffer,
    start: AssemblyOffset,
    start_anchored: AssemblyOffset,
    register_count: usize,
    initial_mem_size: usize,
    visited_set_size: usize,
}

/// State used by the jitted code for execution.
/// It is basically a Vec<u8>, but since it is shared between the jitted
/// code and the rust code we need something lower level, and repr(C)
#[derive(Debug)]
#[repr(C)]
pub struct State {
    /// We use u64 to make sure things are aligned
    mem: *mut u64,
    mem_len: usize,
}

impl Drop for State {
    fn drop(&mut self) {
        // SAFETY: The pointer is owned.
        unsafe {
            alloc::dealloc(
                self.mem as *mut u8,
                Layout::array::<u64>(self.mem_len).unwrap(),
            );
        }
    }
}

impl Clone for State {
    fn clone(&self) -> Self {
        let layout = Layout::array::<u64>(self.mem_len).unwrap();
        // SAFETY:
        // The allocation has nothing in particular.
        // The memcopy works because both pointers are from two different
        // allocations (therefore non overlapping). And they were both allocated
        // with the same length.
        let mem = unsafe {
            let mem = alloc::alloc(layout) as *mut u64;
            std::ptr::copy_nonoverlapping(self.mem, mem, self.mem_len);
            mem
        };

        Self {
            mem,
            mem_len: self.mem_len,
        }
    }
}

// SAFETY:
// State is basically a Vec.
unsafe impl Send for State {}

impl State {
    /// Allocate a new State of the given size in bytes.
    pub fn new(mem_len: usize) -> Self {
        let layout = Layout::array::<u64>(mem_len).unwrap();
        // SAFETY: That's just an allocation.
        let mem = unsafe { alloc::alloc_zeroed(layout) as *mut u64 };
        if mem.is_null() {
            panic!()
        }
        Self { mem, mem_len }
    }

    /// Ensure the given state can hold the given number of bytes,
    /// by reallocating if it is too small.
    pub fn ensure_capacity(&mut self, mem_len: usize) {
        if mem_len > self.mem_len {
            let layout = Layout::array::<u64>(self.mem_len).unwrap();
            let new_mem = unsafe { alloc::realloc(self.mem as *mut u8, layout, mem_len) };

            if new_mem.is_null() {
                panic!()
            }

            self.mem = new_mem as *mut u64;
            self.mem_len = mem_len;
        }
    }

    // pub fn double_size(&mut self) {
    //     self.ensure_capacity(2 * self.mem_len);
    // }

    /// Reset the state for the given regex.
    /// Called before executing.
    pub fn reset(&mut self, pikejit: &JittedRegex) {
        self.ensure_capacity(pikejit.initial_mem_size);

        // SAFETY: TODO
        unsafe {
            std::slice::from_raw_parts_mut(self.mem, pikejit.visited_set_size).fill(0);
        }
    }
}

extern "sysv64" fn _double_mem_size(state: *mut State) -> *mut State {
    // SAFETY: TODO
    // unsafe {
    //     state.as_mut().unwrap_unchecked().double_size();
    // }
    state
}

impl RegexImpl for JittedRegex {
    type State = State;

    fn new_state(&self) -> Self::State {
        State::new(self.initial_mem_size)
    }

    fn reset_state(&self, state: &mut Self::State) {
        state.reset(self);
    }

    fn exec<'s>(&self, input: Input<'s>, state: &mut Self::State, captures: &mut [Span]) -> bool {
        self.exec_internal(&input, state, captures)
    }
}

impl JittedRegex {
    pub fn new(
        pattern: &str,
        config: Config,
    ) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let hir = Parser::from(config.clone()).parse(pattern)?;
        let capture_count = if config.cg {
            hir.properties().explicit_captures_len() + 1
        } else {
            1
        };
        let bytecode = Compiler::compile(hir, config)?;
        let s = if capture_count == 1 {
            PikeJIT::compile::<CGImplReg>(&bytecode, capture_count)?
        } else {
            PikeJIT::compile::<CGImplCowArray>(&bytecode, capture_count)?
        };
        Ok(s)
    }

    fn exec_internal<'s>(&self, input: &Input<'s>, state: &mut State, result: &mut [Span]) -> bool {
        if !input.valid() {
            return false;
        }

        // This assumption is used in the jitted code
        assert!(result.len() <= self.capture_count());

        state.ensure_capacity(self.initial_mem_size);

        let Input {
            subject,
            span,
            first_match,
            anchored,
        } = input;

        let prev_char = find_prev_char(subject, span.from);

        // API:
        // subject: *const u8 -> rdi
        // subject_len: u64 -> rsi
        // result: *mut Span -> rdx
        // result_len: u64 -> rcx
        // state: *mut State -> r8
        // from: u64 -> r9
        // to: u64 -> rbp+8
        // first_match: u64 -> rbp+16
        // prev_char: u32 -> rbp+24
        type ExecSig = extern "sysv64" fn(
            *const u8,
            u64,
            *mut Span,
            u64,
            *mut State,
            u64,
            u64,
            u64,
            Char,
        ) -> u8;

        let f: ExecSig = unsafe {
            if !anchored {
                mem::transmute::<*const u8, ExecSig>(self.code.ptr(self.start))
            } else {
                mem::transmute::<*const u8, ExecSig>(self.code.ptr(self.start_anchored))
            }
        };

        f(
            subject.as_ptr(),
            subject.len() as u64,
            // TODO: This works because of repr(C) but needs something nicer I think
            result.as_mut_ptr(),
            // TODO: This is the length in usize (yeah maybe we should use the array length instead)
            (result.len() * 2) as u64,
            // Pass this as a *mut State
            state as *mut State,
            span.from as u64,
            span.to as u64,
            // TODO: Pass this as a bool instead
            *first_match as u64,
            prev_char,
        ) > 0
    }

    pub(crate) fn capture_count(&self) -> usize {
        self.register_count / 2
    }
}

pub struct PikeJIT {
    ops: Assembler,
    instr_labels: Vec<DynamicLabel>,
    outlined_class_labels: Vec<DynamicLabel>,
    register_count: usize,
    step_next_active: DynamicLabel,
    next_iter: DynamicLabel,
    next_iter_with_search: DynamicLabel,
    fetch_next_char: DynamicLabel,
}

#[derive(Debug)]
pub enum CompileError {
    FailedToCreateAssembler,
    FailedToFinalizeOps,
}

impl Error for CompileError {}

impl Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::FailedToCreateAssembler => {
                write!(f, "Failed to create assembler for the current platform")
            }
            CompileError::FailedToFinalizeOps => write!(f, "Failed to finalize ops"),
        }
    }
}

impl PikeJIT {
    /// Each threads are 2 words long, one word for the pc, and one for
    /// the cg-data (often a pointer).
    const THREAD_SIZE: i32 = 2;
    const THREAD_SIZE_BYTE: i32 = 16;

    pub fn compile<CG: CGImpl>(
        bytecode: &Bytecode,
        capture_count: usize,
    ) -> Result<JittedRegex, CompileError> {
        let mut ops = Assembler::new().map_err(|_| CompileError::FailedToCreateAssembler)?;
        let instr_labels = Vec::from_iter(
            bytecode
                .instructions
                .iter()
                .map(|_| ops.new_dynamic_label()),
        );
        let outlined_class_labels = Vec::from_iter(
            bytecode
                .outlined_classes
                .iter()
                .map(|_| ops.new_dynamic_label()),
        );
        let step_next_active = ops.new_dynamic_label();
        let next_iter = ops.new_dynamic_label();
        let next_iter_with_search = ops.new_dynamic_label();
        let fetch_next_char = ops.new_dynamic_label();
        let mut compiler = Self {
            ops,
            instr_labels,
            register_count: capture_count * 2,
            step_next_active,
            next_iter,
            next_iter_with_search,
            fetch_next_char,
            outlined_class_labels,
        };
        for (i, class) in bytecode.outlined_classes.iter().enumerate() {
            compiler.compile_outlined_class(i, class);
        }
        for (i, instr) in bytecode.instructions.iter().enumerate() {
            let barrier = bytecode.barriers[i];
            compiler.compile_instruction::<CG>(i, instr, barrier);
        }
        compiler.assemble::<CG>()
    }

    fn ensure_mem_capacity(&mut self, additional_space: usize) {
        // __!(self.ops,
        // mov reg1, [rbp + mem_size_offset!()]
        // )
    }

    fn assemble<CG: CGImpl>(mut self) -> Result<JittedRegex, CompileError> {
        let label0 = self.instr_labels[0];
        let start;
        let start_anchored;
        __!(self.ops,
         ; start_anchored = self.ops.offset()
         ;; self.prologue::<CG>()
         ;; self.push_active_sentinel(self.next_iter)
         ;; CG::alloc_thread(&mut self)
         ;; CG::write_reg(&mut self, 0)
         ;; self.push_active(label0)
         ; jmp =>self.fetch_next_char
         ;; start = self.ops.offset()
         ;; self.prologue::<CG>()
         ;; self.push_active_sentinel(self.next_iter_with_search)
         ;; CG::alloc_thread(&mut self)
         ;; CG::write_reg(&mut self, 0)
         ;; self.push_active(label0)
         ; =>self.fetch_next_char
         ; mov prev_char, curr_char
         ; cmp input_len, input_pos
         ; je >input_end
         ;; self.decode_next_utf_8()
         ; jmp =>self.step_next_active
         ; input_end:
         // This character is not a valid utf-8 char and therefore it is fine to
         // use it to encode the end of input. This works because both the regex
         // and the input must only contain valid utf-8 chars.
         ; mov curr_char, Char::INPUT_BOUND.into()

         // The dispatch loop, simply pop active and dispatch
         ; =>self.step_next_active
         ;; self.pop_active()
         ; jmp reg1

         // Called at then end of an iteration, meaning we step through all
         // threads in active, or we reached an accepting states (and therefore
         // emptied the active queue)
         // This version is used in unanchored searches, where we spawn a new
         // thread everytime we start a new iteration
         ; =>self.next_iter_with_search
         ; cmp input_pos, span_end
         ; je >return_result
         ; add input_pos, input_inc
         ; mov curr_top, [rbp + (next_tail_init_offset!())]
         ;; CG::alloc_thread(&mut self)
         ;; CG::write_reg(&mut self, 0)
         ;; self.push_next(label0)
         ;; self.push_next_sentinel(self.next_iter_with_search)
         // Share at least the end with next_iter
         ; jmp >next
         // Same as above, but does not spawn a new thread. Used when doing
         // anchored searches, or when an accpeting state has already been
         // reached.
         ; =>self.next_iter
         ; cmp input_pos, span_end
         ; je >return_result
         ; add input_pos, input_inc
         ; mov curr_top, [rbp + (next_tail_init_offset!())]
         // Check if next is empty
         ; cmp curr_top, next_tail
         ; je >return_result
         ;; self.push_next_sentinel(self.next_iter)
         ; next:
         ; mov next_tail, [rbp + (curr_top_init_offset!())]
         ; mov [rbp + (curr_top_init_offset!())], curr_top
         ; mov [rbp + (next_tail_init_offset!())], next_tail
         ; jmp =>self.fetch_next_char
         ; return_result:
         ;; CG::return_result(&mut self)
         ;; CG::at_code_end(&mut self)
        );

        let visited_set_size = self.visited_set_size();
        let initial_mem_size = self.initial_mem_size::<CG>();
        let code = self.ops.finalize().unwrap();

        Ok(JittedRegex {
            code,
            start,
            start_anchored,
            register_count: self.register_count,
            visited_set_size,
            initial_mem_size,
        })
    }

    fn pop_active(&mut self) {
        __!(self.ops,
          sub curr_top, Self::THREAD_SIZE_BYTE
        ; mov curr_thd_data, QWORD [curr_top]
        ; mov reg1, QWORD [curr_top + 8]
        )
    }

    fn push_active(&mut self, label: DynamicLabel) {
        __!(self.ops,
          mov QWORD [curr_top], curr_thd_data
        ; lea reg1, [=>label]
        ; mov QWORD [curr_top + 8], reg1
        ; add curr_top, Self::THREAD_SIZE_BYTE
        )
    }

    fn push_active_sentinel(&mut self, label: DynamicLabel) {
        __!(self.ops,
          xor curr_thd_data, curr_thd_data
        );
        self.push_active(label);
    }

    fn push_next_sentinel(&mut self, label: DynamicLabel) {
        __!(self.ops,
          xor curr_thd_data, curr_thd_data
        );
        self.push_next(label);
    }

    fn push_next(&mut self, label: DynamicLabel) {
        __!(self.ops,
          sub next_tail, Self::THREAD_SIZE_BYTE
        ; mov QWORD [next_tail], curr_thd_data
        ; lea reg1, [=>label]
        ; mov QWORD [next_tail + 8], reg1
        )
    }

    /* The overall shape of the memory is the following:
     * |---------visited_set--------|-------queue_1------|-----queue2------|--------cg_space--------|
     */

    /// Returns the size in words (okok in x64 words are 16bit, but here we mean 64bit)
    /// Basically in sizeof::<usize>() if you will.
    /// Everything in the state must be 8 bytes aligned, therefore memory size is
    /// measured in multiples of 8bytes.
    /// However offsets are in bytes, since those are directly used in mov instructions
    fn visited_set_size(&self) -> usize {
        self.instr_labels.len()
    }

    /// Starting offset of the queues region (in *bytes*)
    fn queue_start(&self) -> usize {
        self.visited_set_size() * ptr_size!()
    }

    fn queue_size(&self) -> usize {
        (self.instr_labels.len() * 2 + 1) * Self::THREAD_SIZE as usize
    }

    fn total_queue_size(&self) -> usize {
        self.queue_size() * 2
    }

    fn initial_mem_size<CG: CGImpl>(&self) -> usize {
        self.visited_set_size() + self.total_queue_size() + CG::init_mem_size(self)
    }

    fn cg_mem_start(&self) -> usize {
        (self.visited_set_size() + self.total_queue_size()) * ptr_size!()
    }

    fn max_concurrent_threads(&self) -> usize {
        // This is an upperbound, it is a bit less in practice
        3 * self.instr_labels.len()
    }

    fn prologue<CG: CGImpl>(&mut self) {
        __!(self.ops,
          push rbp
        ; mov rbp, rsp
        // Push result_ptr
        ; push rdx
        // Push result_length
        ; push rcx
        // Push saved registers
        ; push rbx
        ; push r12
        ; push r13
        ; push r14
        ; push r15
        // Initialize mem, input_pos, input_end, state_ptr and prev_char
        ; mov [rbp + state_ptr_offset!()], r8
        // State is { mem: *mut u64, size: usize }, and is repr(c)
        ; mov mem, [r8]
        ; mov input_pos, r9
        ; mov span_end, [rbp + span_end_offset!()]
        // We set curr_char, because the first thing we do after the prologue is to
        // swap curr_char with prev_char, and fetch the next char in curr_char
        ; mov curr_char, [rbp + prev_char_offset!()]
        // Okay so push immediate does not support 64bits value with this
        // library. Therefore we do it manually.
        ; sub rsp, (4*ptr_size!())
        // Initialize curr_top, next_tail and saved them on the stack for easier
        // swapping. Okay movabs is broken
        // TODO FIX THIS
        ; mov rax, ((self.queue_start() + ((self.queue_size() * ptr_size!())/2)) as i32)
        ; add rax, mem
        ; mov QWORD [rbp + (curr_top_init_offset!())], rax
        ; mov curr_top, rax
        ; mov rax, ((self.queue_start() + ((3*ptr_size!()*self.queue_size())/2)) as i32)
        // TODO: I think we need to decrement this to let a space for the sentinel
        ; add rax, mem
        ; mov QWORD [rbp + (next_tail_init_offset!())], rax
        ; mov next_tail, rax
        ;; CG::initialize_cg_region(self)
        )
    }

    fn check_has_visited<CG: CGImpl>(&mut self, instr_index: usize) {
        // This limit the size of the input string,
        // The better way would be to load with an immediate
        // offset if instr_index fits in 31 bits, otherwise
        // load it in a base register with a scaling factor
        let byte_offset = ((instr_index * 8) as u32).cast_signed();
        __!(self.ops,
          mov reg1, [mem + byte_offset]
        ; cmp reg1, input_pos
        // The idea is that when writing in visited, we write input_pos + 1
        // That way 0 (which is what the memory is initialized to) can always
        // be crossed.
        ; jbe >success
        ;; CG::free_curr_thread(self)
        ; jmp =>self.step_next_active
        ; success:
        ; lea reg1, [input_pos + 1]
        ; mov [mem + byte_offset], reg1
        )
    }

    fn epilogue(&mut self) {
        __!(self.ops,
          mov rbx, [rbp + saved_rbx_offset!()]
        ; mov r12, [rbp + saved_r12_offset!()]
        ; mov r13, [rbp + saved_r13_offset!()]
        ; mov r14, [rbp + saved_r14_offset!()]
        ; mov r15, [rbp + saved_r15_offset!()]
        ; mov rsp, rbp
        ; pop rbp
        )
    }

    fn compile_instruction<CG: CGImpl>(
        &mut self,
        i: usize,
        instruction: &Instruction,
        barrier: bool,
    ) {
        self.bind_label(i);
        if barrier {
            self.check_has_visited::<CG>(i);
        }
        match instruction {
            Instruction::Consume(c) => self.compile_consume::<CG>(i, *c),
            Instruction::ConsumeAny => self.compile_consume_any(i),
            Instruction::ConsumeClass(class) => self.compile_consume_class::<CG>(i, class),
            Instruction::Fork2(a, b) => self.compile_fork::<CG>(&[*a, *b]),
            Instruction::ForkN(items) => self.compile_fork::<CG>(items),
            Instruction::Jmp(target) => self.compile_jump(*target),
            Instruction::WriteReg(reg) => self.compile_write_reg::<CG>(i, *reg),
            Instruction::Accept => self.compile_accept::<CG>(),
            Instruction::ConsumeOutlined(class_id) => {
                self.compile_consume_outlined::<CG>(i, *class_id)
            }
            Instruction::Assertion(look) => self.compile_assertion::<CG>(i, *look),
        }
    }

    fn compile_consume_outlined<CG: CGImpl>(&mut self, i: usize, class_id: usize) {
        let class_label = self.outlined_class_labels[class_id];
        __!(self.ops,
          call =>class_label
        ; test reg1, reg1
        ; jnz >fail
        ;; self.push_next(self.instr_labels[i+1])
        ; jmp =>self.step_next_active
        ; fail:
        ;; CG::free_curr_thread(self)
        ; jmp =>self.step_next_active
        )
    }

    fn compile_outlined_class(&mut self, i: usize, class: &[(Char, Char)]) {
        let label = self.outlined_class_labels[i];

        __!(self.ops, =>label
        // I don't have any actual evidence for that "fast path", it just seems
        // slow that for very large classes, if we are at the end of the input
        // we compare this sentinel value agains every interval even though we
        // know it will never match.
        ; cmp curr_char, Char::INPUT_BOUND.into()
        ; jne >next
        ; mov reg1, 1
        ; ret
        ; next:
        );
        for (from, to) in class {
            __!(self.ops,
                cmp curr_char, (u32::from(*from)).cast_signed()
            ; jb >fail
            ; cmp curr_char, (u32::from(*to)).cast_signed()
            ; ja >next
            ; mov reg1, 0
            ; ret
            ; fail:
            ; mov reg1, 1
            ; ret
            ; next:
            )
        }
        __!(self.ops,
          mov reg1, 1
        ; ret
        )
    }

    fn bind_label(&mut self, i: usize) {
        let label = self.instr_labels[i];
        __!(self.ops, =>label)
    }

    fn compile_consume<CG: CGImpl>(&mut self, i: usize, c: Char) {
        let next_label = self.instr_labels[i + 1];
        __!(self.ops,
          cmp curr_char, ((u32::from(c)).cast_signed())
        ; jne >fail
        ;; self.push_next(next_label)
        ; jmp =>self.step_next_active
        ; fail:
        ;; CG::free_curr_thread(self)
        ; jmp =>self.step_next_active
        )
    }

    fn compile_consume_any(&mut self, i: usize) {
        let next_label = self.instr_labels[i + 1];
        self.push_next(next_label);
        __!(self.ops, jmp =>self.step_next_active)
    }

    fn compile_consume_class<CG: CGImpl>(&mut self, i: usize, class: &[(Char, Char)]) {
        let fail = self.ops.new_dynamic_label();
        let next = self.ops.new_dynamic_label();
        for (from, to) in class {
            self.compile_consume_range(next, fail, *from, *to);
        }
        __!(self.ops,
          =>fail
        ;; CG::free_curr_thread(self)
        ; jmp =>self.step_next_active
        ; =>next
        ;; self.push_next(self.instr_labels[i+1])
        ; jmp =>self.step_next_active
        )
    }

    fn compile_consume_range(
        &mut self,
        next_label: DynamicLabel,
        fail_label: DynamicLabel,
        from: Char,
        to: Char,
    ) {
        __!(self.ops,
          cmp curr_char, (u32::from(from)).cast_signed()
        ; jb =>fail_label
        ; cmp curr_char, (u32::from(to)).cast_signed()
        ; ja >next
        ; jmp =>next_label
        ; next:
        )
    }

    fn compile_fork<CG: CGImpl>(&mut self, branches: &[usize]) {
        let len = branches.len();
        for i in (1..len).rev() {
            let instr_i = branches[i];
            self.push_active(self.instr_labels[instr_i]);
            CG::clone_curr_thread(self);
        }
        __!(self.ops, jmp => self.instr_labels[branches[0]])
    }

    fn compile_jump(&mut self, target: usize) {
        let label = self.instr_labels[target];
        __!(self.ops, jmp => label)
    }

    fn compile_write_reg<CG: CGImpl>(&mut self, i: usize, reg: u32) {
        CG::write_reg(self, reg);
        let next_label = self.instr_labels[i + 1];
        __!(self.ops, jmp =>next_label)
    }

    fn compile_accept<CG: CGImpl>(&mut self) {
        CG::accept_curr_thread(self);
        __!(self.ops,
          mov reg1, [rbp + return_on_accept!()]
        ; test reg1, reg1
        ; jnz >return_result
        // Note: Here we jumpt to next_iter without starting a new thread
        ; jmp => self.next_iter
        ; return_result:
        ;; CG::return_result(self)
        )
    }

    /// Decode the next character in the input in curr_char, and write the
    /// number of bytes consumed in input_inc.
    /// ## Note
    /// It assumes the input is valid utf-8 (which is always true in rust)
    /// and it does not perform any bound check.
    fn decode_next_utf_8(&mut self) {
        __!(self.ops,
          movzx curr_char, BYTE [input + input_pos]      // Load first byte
        ; cmp curr_char, 0x80
        ; jb >ascii
        ; cmp curr_char, 0xE0
        ; jb >twobyte
        ; cmp curr_char, 0xF0
        ; jb >threebyte
        ; jmp >fourbyte

        ; ascii:
        ; mov input_inc, 1
        ; jmp >done

        ; twobyte:
        // Decode 2-byte sequence: 110xxxxx 10xxxxxx
        ; and curr_char, (0b11111)
        ; shl curr_char, 6
        ; movzx reg1d, BYTE [input + input_pos + 1] // Load second byte
        ; and reg1d, 0b111111
        ; or curr_char, reg1d
        ; mov input_inc, 2
        ; jmp >done

        ; threebyte:
        // Decode 3-byte sequence: 1110xxxx 10xxxxxx 10xxxxxx
        ; and curr_char, (0b1111)
        ; shl curr_char, 12
        ; movzx reg1d, BYTE [input + input_pos + 1] // Load second byte
        ; and reg1d, 0b111111
        ; shl reg1d, 6
        ; or curr_char, reg1d
        ; movzx reg1d, BYTE [input + input_pos + 2] // Load third byte
        ; and reg1d, 0b111111
        ; or curr_char, reg1d
        ; mov input_inc, 3
        ; jmp >done

        ; fourbyte:
        // Decode 4-byte sequence: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
        ; and curr_char, (0b111)
        ; shl curr_char, 18
        ; movzx reg1d, BYTE [input + input_pos + 1] // Load second byte
        ; and reg1d, 0b111111
        ; shl reg1d, 12
        ; or curr_char, reg1d
        ; movzx reg1d, BYTE [input + input_pos + 2] // Load third byte
        ; and reg1d, 0b111111
        ; shl reg1d, 6
        ; or curr_char, reg1d
        ; movzx reg1d, BYTE [input + input_pos + 3] // Load fourth byte
        ; and reg1d, 0b111111
        ; or curr_char, reg1d
        ; mov input_inc, 4

        ; done:
        )
    }

    fn compile_assertion<CG: CGImpl>(&mut self, i: usize, look: Look) {
        match look {
            Look::Start => {
                __!(self.ops,
                  cmp prev_char,  Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active
                )
            }
            Look::End => {
                __!(self.ops,
                  cmp curr_char, Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active
                )
            }
            Look::StartLF => {
                __!(self.ops,
                  cmp prev_char,  Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ; cmp prev_char,  ('\n' as u32).cast_signed()
                ; je =>self.instr_labels[i+1]
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active
                )
            }
            Look::EndLF => {
                __!(self.ops,
                  cmp curr_char,  Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ; cmp curr_char,  ('\n' as u32).cast_signed()
                ; je =>self.instr_labels[i+1]
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active
                )
            }
            Look::StartCRLF => {
                __!(self.ops,
                  cmp prev_char,  Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ; cmp prev_char,  ('\n' as u32).cast_signed()
                ; je =>self.instr_labels[i+1]
                ; cmp prev_char,  ('\r' as u32).cast_signed()
                ; jne >fail
                ; cmp curr_char,  ('\n' as u32).cast_signed()
                ; jne =>self.instr_labels[i+1]
                ; fail:
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active
                )
            }
            Look::EndCRLF => {
                __!(self.ops,
                  cmp curr_char,  Char::INPUT_BOUND.into()
                ; je =>self.instr_labels[i+1]
                ; cmp curr_char,  ('\r' as u32).cast_signed()
                ; je =>self.instr_labels[i+1]
                ; cmp curr_char,  ('\n' as u32).cast_signed()
                ; jne >fail
                ; cmp prev_char,  ('\r' as u32).cast_signed()
                ; jne =>self.instr_labels[i+1]
                ; fail:
                ;; CG::free_curr_thread(self)
                ; jmp =>self.step_next_active)
            }
            _ => todo!(),
        }
    }
}
