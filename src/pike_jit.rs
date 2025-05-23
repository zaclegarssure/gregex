use std::mem;

use cg_implementation::CGImpl;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, dynasm, x64::Assembler};
use regex_syntax::hir::Class;

use crate::{pike_bytecode::Instruction, regexp_match::Match};

macro_rules! __ {
    ($ops: expr, $($t:tt)*) => {
        dynasm!($ops
        ; .arch x64
        ; .alias retval, rax
        ; .alias curr_thd_data, rax
        ; .alias mem, r8
        ; .alias curr_char, ecx
        ; .alias input_pos, rdx
        ; .alias input, rdi
        ; .alias input_len, rsi
        ; .alias next_tail, r9
        ; .alias curr_top, r10
        ; .alias cg_reg, r11
        ; .alias prev_char, r12
        ; .alias reg1, r13
        ; .alias reg1d, r13d
        ; .alias reg2, r14
        ; .alias reg2d, r14d
        ; .alias input_inc, r15
        ; $($t)*
        )
    };
}

macro_rules! cst {
    ($name: ident, $value: expr) => {
        macro_rules! $name {
            () => {
                $value
            };
        }
    };
}

cst!(ptr_size, 8);
cst!(frame_ptr_offset, 0);
cst!(return_addr_offset, frame_ptr_offset!() + ptr_size!());
cst!(result_offset, frame_ptr_offset!() - ptr_size!());
cst!(result_length_offset, result_offset!() - ptr_size!());
cst!(saved_r12_offset, result_length_offset!() - ptr_size!());
cst!(saved_r13_offset, saved_r12_offset!() - ptr_size!());
cst!(saved_r14_offset, saved_r13_offset!() - ptr_size!());
cst!(saved_r15_offset, saved_r14_offset!() - ptr_size!());
cst!(next_tail_init_offset, saved_r15_offset!() - ptr_size!());
cst!(curr_top_init_offset, next_tail_init_offset!() - ptr_size!());
cst!(mem_size_offset, curr_top_init_offset!() - ptr_size!());
cst!(last_saved_value_offset, mem_size_offset!());

pub mod cg_impl_array;
pub mod cg_impl_register;
pub mod cg_implementation;

pub struct JittedRegex {
    code: dynasmrt::ExecutableBuffer,
    start: dynasmrt::AssemblyOffset,
    register_count: usize,
    initial_mem_size: usize,
    reusable_mem: Option<Box<[u8]>>,
}

impl JittedRegex {
    pub fn exec<'s>(&self, subjetc: &'s str) -> Option<Match<'s>> {
        let f: extern "sysv64" fn(*const u8, u64, *mut i64, u64, *mut u8) -> u8 =
            unsafe { mem::transmute(self.code.ptr(self.start)) };
        let mut result = vec![-1; self.register_count];
        let mut mem = vec![0; self.initial_mem_size];
        if f(
            subjetc.as_ptr(),
            subjetc.len() as u64,
            result.as_mut_ptr(),
            self.register_count as u64,
            mem.as_mut_ptr(),
        ) > 0
        {
            let mut indices = vec![None; self.register_count];
            for (i, pair) in result.chunks(2).enumerate() {
                let lower = pair[0];
                let upper = pair[1];
                if upper >= 0 && lower >= 0 {
                    indices[2 * i] = Some(lower as usize);
                    indices[2 * i + 1] = Some(upper as usize);
                } else {
                    println!("lower = {lower} upper = {upper}");
                }
            }
            Some(Match::new(subjetc, indices.into_boxed_slice()))
        } else {
            None
        }
    }
}

pub struct PikeJIT {
    ops: Assembler,
    // Might just always be 0 idk
    start: dynasmrt::AssemblyOffset,
    instr_labels: Vec<DynamicLabel>,
    register_count: usize,
    step_next_active: DynamicLabel,
    next_iter: DynamicLabel,
    fetch_next_char: DynamicLabel,
}

#[derive(Debug)]
pub enum CompileError {
    FailedToCreateAssembler,
}

impl PikeJIT {
    // 8 bytes for the thread data and 8 for the code location
    // Those 8 bytes could be compressed into 4 by saving
    // just the offset of the code location.
    const THREAD_SIZE: i32 = 16;
    pub fn compile<CG: CGImpl>(
        bytecode: &[Instruction],
        register_count: usize,
    ) -> Result<JittedRegex, CompileError> {
        let mut ops = Assembler::new().map_err(|_| CompileError::FailedToCreateAssembler)?;
        let instr_labels = Vec::from_iter(bytecode.iter().map(|_| ops.new_dynamic_label()));
        let start = ops.offset();
        let step_next_active = ops.new_dynamic_label();
        let next_iter = ops.new_dynamic_label();
        let fetch_next_char = ops.new_dynamic_label();
        let mut compiler = Self {
            ops,
            start,
            instr_labels,
            register_count,
            step_next_active,
            next_iter,
            fetch_next_char,
        };
        let ops = &mut compiler.ops;
        // In practice we could call directly start_label when calling the jitted code
        __!(ops, jmp ->start_label);
        for (i, instr) in bytecode.iter().enumerate() {
            compiler.compile_instruction::<CG>(i, instr);
        }
        compiler.assemble::<CG>()
    }

    fn assemble<CG: CGImpl>(mut self) -> Result<JittedRegex, CompileError> {
        let label0 = self.instr_labels[0];
        // API: exec(input: rdi *u8, input_len: rsi usize, result: rdx *u64, result_len: rcx usize, mem: r8 u8*)
        //          -> (match_count: rax u64)
        __!(self.ops,
         ->start_label:
         ;; self.prologue::<CG>()
         ;; self.push_active_sentinel(self.next_iter)
         ;; CG::alloc_thread(&mut self)
         ;; self.push_active(label0)
         ; =>self.fetch_next_char
         ; cmp input_len, input_pos
         ; je >input_end
         ;; self.decode_next_utf_8()
         ; jmp =>self.step_next_active
         ; input_end:
         // This character is not a valid utf-8 char and therefore it is fine to
         // use it to encode the end of input. This works because both the regex
         // and the input must only contain valid utf-8 chars.
         ; mov curr_char, u32::MAX.cast_signed()
         ; =>self.step_next_active
         ;; self.pop_active()
         ; jmp reg1
         ; =>self.next_iter
         ; cmp input_pos, input_len
         ; je >return_result
         ; mov curr_top, [rbp + (next_tail_init_offset!())]
         ; cmp curr_top, next_tail
         ; je >return_result
         ;; self.push_next_sentinel(self.next_iter)
         ; mov next_tail, [rbp + (curr_top_init_offset!())]
         ; mov [rbp + (curr_top_init_offset!())], curr_top
         ; mov [rbp + (next_tail_init_offset!())], next_tail
         ; add input_pos, input_inc
         ; jmp =>self.fetch_next_char
         ; return_result:
         ;; CG::return_result(&mut self)
         ;; CG::at_code_end(&mut self)
        );

        let initial_mem_size = self.initial_mem_size::<CG>();
        let code = self.ops.finalize().unwrap();
        Ok(JittedRegex {
            code,
            start: self.start,
            register_count: self.register_count,
            initial_mem_size,
            reusable_mem: None,
        })
    }

    fn pop_active(&mut self) {
        __!(self.ops,
          sub curr_top, Self::THREAD_SIZE
        ; mov curr_thd_data, QWORD [curr_top]
        ; mov reg1, QWORD [curr_top + 8]
        )
    }

    fn push_active(&mut self, label: DynamicLabel) {
        __!(self.ops,
          mov QWORD [curr_top], curr_thd_data
        ; lea reg1, [=>label]
        ; mov QWORD [curr_top + 8], reg1
        ; add curr_top, Self::THREAD_SIZE
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
          sub next_tail, Self::THREAD_SIZE
        ; mov QWORD [next_tail], curr_thd_data
        ; lea reg1, [=>label]
        ; mov QWORD [next_tail + 8], reg1
        )
    }

    /* The overall shape of the memory is the following:
     * |---------visited_set--------|-------queue_1------|-----queue2------|--------cg_space--------|
     */

    fn visited_set_size(&self) -> usize {
        self.instr_labels.len() * ptr_size!()
    }

    fn queue_start(&self) -> usize {
        self.visited_set_size()
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
        self.visited_set_size() + self.total_queue_size()
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
        ; push r12
        ; push r13
        ; push r14
        ; push r15
        // Okay so push immediate does not support 64bits value with
        // this library, therefore I'm afraid it will not properly decrement
        // rsp enough to reserve the right amount of space.
        // Therefore we do it manually.
        ; sub rsp, (4*ptr_size!())
        // This forces the initial memory to be roughly 2GB, in a quite artificial way
        // we could easily remove this limit.
        // First instead of casting we should bit-cast to i32 to gain 1 bit
        // Also we should use movabsq
        ; lea reg1, [mem + ((self.queue_start() + (self.queue_size()/2)) as i32)]
        ; mov QWORD [rbp + (next_tail_init_offset!())], reg1
        ; lea reg1, [mem + ((self.queue_start() + ((3*self.queue_size())/2)) as i32)]
        ; mov QWORD [rbp + (curr_top_init_offset!())], reg1
        ; mov QWORD [rbp + (mem_size_offset!())], (self.initial_mem_size::<CG>() as i32)
        // Initialize curr_top and next_tail
        ; mov curr_top, (self.queue_start() as i32)
        ; add curr_top, mem
        ; mov next_tail, [rbp + (next_tail_init_offset!())]
        // Initialize input_pos
        ; mov input_pos, 0
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
          mov r12, [rbp + saved_r12_offset!()]
        ; mov r13, [rbp + saved_r13_offset!()]
        ; mov r14, [rbp + saved_r14_offset!()]
        ; mov r15, [rbp + saved_r15_offset!()]
        ; mov rsp, rbp
        ; pop rbp
        )
    }

    fn compile_instruction<CG: CGImpl>(&mut self, i: usize, instr: &Instruction) {
        self.bind_label(i);
        self.check_has_visited::<CG>(i);
        match instr {
            Instruction::Consume(c) => self.compile_consume::<CG>(i, *c),
            Instruction::ConsumeAny => self.compile_consume_any(i),
            Instruction::ConsumeClass(class) => self.compile_consume_class::<CG>(i, class),
            Instruction::Fork2(a, b) => self.compile_fork::<CG>(&[*a, *b]),
            Instruction::ForkN(items) => self.compile_fork::<CG>(items.as_slice()),
            Instruction::Jmp(target) => self.compile_jump(*target),
            Instruction::WriteReg(reg) => self.compile_write_reg::<CG>(i, *reg),
            Instruction::Accept => self.compile_accept::<CG>(),
        }
    }

    fn bind_label(&mut self, i: usize) {
        let label = self.instr_labels[i];
        __!(self.ops, =>label)
    }

    fn compile_consume<CG: CGImpl>(&mut self, i: usize, c: char) {
        let next_label = self.instr_labels[i + 1];
        __!(self.ops,
          cmp curr_char, ((c as u32).cast_signed())
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

    fn compile_consume_class<CG: CGImpl>(&mut self, i: usize, class: &Class) {
        let fail = self.ops.new_dynamic_label();
        let next = self.ops.new_dynamic_label();
        match class {
            Class::Unicode(class_unicode) => {
                for class in class_unicode.iter() {
                    let from = class.start();
                    let to = class.end();
                    self.compile_consume_range(next, fail, from, to);
                }
            }
            Class::Bytes(class_bytes) => {
                for class in class_bytes.iter() {
                    let from = class.start();
                    let to = class.end();
                    self.compile_consume_range(next, fail, from as char, to as char);
                }
            }
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
        from: char,
        to: char,
    ) {
        __!(self.ops,
          cmp curr_char, (from as u32).cast_signed()
        ; jb =>fail_label
        ; cmp curr_char, (to as u32).cast_signed()
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
        __!(self.ops, jmp => self.next_iter);
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
}
