use dynasmrt::{DynasmApi, DynasmLabelApi, dynasm};

use crate::util::Span;

use super::{PikeJIT, cg_implementation::CGImpl};

/// CG implementation using trees. All operations are O(1), and threads don't
/// need to be freed. However this consumes O(|haystack|) memory
pub struct CGImplTree;

#[repr(C)]
struct Node {
    prev: usize,
    pos: usize,
    reg: usize,
}

// The result is just a pointer to the last cg-operation and the close cg0
// of the winning thread. Or 0 if no thread won.
// The last value is just a boolean to say if we have a result or not
cst!(
    current_match_offset,
    last_saved_value_offset!() - 3 * ptr_size!()
);

extern "sysv64" fn write_results(
    spans: *mut Span,
    reg_count: usize,
    mut offset: usize,
    mem: *const u64,
    cg0_to: usize,
) {
    unsafe {
        // Reset span array
        // CG0 is always set
        for i in 1..(reg_count / 2) {
            (*spans.add(i)) = Span::invalid();
        }
        loop {
            if (offset as isize) <= 0 {
                break;
            }
            let tree = std::mem::transmute::<*const u64, *const Node>(mem.add(offset / 8));
            let reg = (*tree).reg;
            let pos = (*tree).pos;
            let span_idx = reg / 2;
            if reg < reg_count {
                let span = spans.add(span_idx);
                if (*span).from == usize::MAX {
                    if reg % 2 == 0 {
                        (*span).from = pos;
                    } else {
                        (*span).to = pos;
                    }
                }
            }
            offset = (*tree).prev;
        }
        // Last group is always cg 0
        let pos = (-(offset as isize)) as usize;
        (*spans).from = pos;
        (*spans).to = cg0_to;
    }
}

impl CGImpl for CGImplTree {
    fn write_reg(jit: &mut PikeJIT, reg: u32) {
        if reg == 0 {
            // Since curr_thd_data is init to 0, we set it to -inpu_pos, that way it is flagged as being cg_0
            // and we avoid using memory
            __!(jit.ops,
             sub curr_thd_data, input_pos
            );
            return;
        } else if reg == 1 {
            unreachable!()
        }
        __!(jit.ops,
          mov [mem + cg_reg], curr_thd_data
        ; mov [mem + cg_reg + 8], input_pos
        ; mov QWORD [mem + cg_reg + 16], reg as i32
        ; mov curr_thd_data, cg_reg
        ; add cg_reg, size_of::<Node>() as i32
        )
    }

    fn accept_curr_thread(jit: &mut PikeJIT) {
        __!(jit.ops,
        // Record the closing group directly in the result (like with cg-reg)
          mov [rbp + current_match_offset!() + ptr_size!()], input_pos
        // Set to 1 to indicate there is a match
        ; mov QWORD [rbp + current_match_offset!() + 2*ptr_size!()], 1
        ; mov [rbp + current_match_offset!()], curr_thd_data
        )
    }

    #[allow(clippy::fn_to_numeric_cast)]
    fn return_result(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov rdx, [rbp + current_match_offset!() + 2*ptr_size!()]
        ; test rdx, rdx
        ; jz >no_match
        ; mov rdx, [rbp + current_match_offset!()]
        ; mov rdi, [rbp + result_offset!()]
        ; mov rsi, [rbp + result_len_offset!()]
        ; mov rcx, mem
        ; mov r8, [rbp + current_match_offset!() + ptr_size!()]
        ; mov rax, QWORD write_results as _
        // TODO: Check the alignment but normally it should be good
        ; call rax
        ; mov rax, 1
        ;; jit.epilogue()
        ; ret
        ; no_match:
        ; mov rax, 0
        ;; jit.epilogue()
        ; ret
        )
    }

    fn init_mem_size(jit: &PikeJIT) -> usize {
        // TODO: Figure out some goods bounds
        // TODO: Make it depends on the number of write_cg ops
        jit.write_reg_count * (size_of::<Node>() / size_of::<usize>()) * 4
    }

    fn initialize_cg_region(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov cg_reg, QWORD jit.cg_mem_start() as _
        // Mark that no match were found yet
        ; mov QWORD [rbp + current_match_offset!() + 2*ptr_size!()], 0
        ;; jit.set_and_align_sp(current_match_offset!())
        )
    }

    fn alloc_thread(jit: &mut PikeJIT) {
        __!(jit.ops, xor curr_thd_data, curr_thd_data);
    }

    fn free_curr_thread(_jit: &mut PikeJIT) {}

    fn clone_curr_thread(_jit: &mut PikeJIT) {}

    fn at_code_end(_jit: &mut PikeJIT) {}

    fn at_fetch_next_char(jit: &mut PikeJIT) {
        let requested_space = (jit.write_reg_count * size_of::<Node>()) as i32;
        __!(jit.ops,
        // This incurr a load from the stack at every iter which would be great to not have
          mov reg1, [rbp + state_ptr_offset!()]
        ; mov reg2, [reg1 + ptr_size!()]
        // We want the size in byte, not in word, hence the shift
        ; shl reg2, 3
        ; sub reg2, cg_reg
        ; cmp reg2, requested_space
        ; jae >enough_space
        // Prepare calling external function, push saved registers
        ; push rax
        ; push rcx
        ; push rdx
        ; push rsi
        ; push rdi
        ; push r11
        ; push r9
        ; push r10
        ; mov rdi, reg1
        // TODO: Again, make sure rsp is 16byte aligned, but nomrally it should
        ;; jit.grow_memory()
        // Reload mem
        ; mov mem, [rax]
        // Pop saved registers
        ; pop r10
        ; pop r9
        ; pop r11
        ; pop rdi
        ; pop rsi
        ; pop rdx
        ; pop rcx
        ; pop rax
        ; enough_space:
        )
    }
}
