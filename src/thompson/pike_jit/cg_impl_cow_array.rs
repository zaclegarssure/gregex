use dynasmrt::{DynasmApi, DynasmLabelApi, dynasm};

use super::{PikeJIT, cg_implementation::CGImpl};

pub struct CGImplCowArray;

impl CGImplCowArray {
    fn free_list_size(jit: &PikeJIT) -> usize {
        jit.max_concurrent_threads() + 2
    }

    fn array_size(jit: &PikeJIT) -> usize {
        // +1 for the rc count
        jit.register_count + 1
    }

    fn free_all_threads_in_active(jit: &mut PikeJIT) {
        __!(jit.ops,
          loop_:
        ;; jit.pop_active()
        ; test curr_thd_data, curr_thd_data
        // The sentinel thread (the one switching to the next iteration)
        // has 0 as it's thread data.
        ; jz >end
        ;; Self::free_curr_thread(jit)
        ; jmp <loop_
        ; end:
        )
    }
}

// The match is only saved as a pointer to the array
// containing it. Therefore it's 64 bits long.
cst!(
    current_match_offset,
    last_saved_value_offset!() - (ptr_size!())
);

impl CGImpl for CGImplCowArray {
    fn write_reg(jit: &mut PikeJIT, reg: u32) {
        let offset = (jit.register_count * ptr_size!()) as i32;
        __!(jit.ops,
        // Load rc
          mov reg1, [mem + curr_thd_data + offset]
        // If == 1 then no need to copy the array
        ; cmp reg1, 1
        ; je >next
        // Decrement ref-count
        ; dec reg1
        ; mov [mem + curr_thd_data + offset], reg1
        ; call ->clone_array
        ; next:
        ; mov QWORD [mem + curr_thd_data + ((reg*ptr_size!()) as i32)], input_pos
        )
    }

    fn accept_curr_thread(jit: &mut PikeJIT) {
        Self::write_reg(jit, 1);
        __!(jit.ops,
          mov reg1, [rbp + current_match_offset!()]
        ; push reg1
        ; mov [rbp + current_match_offset!()], curr_thd_data
        ;; Self::free_all_threads_in_active(jit)
        ; pop reg1
        ; test reg1, reg1
        ; jz >end
        // TODO Find a way to pass a register to free_curr_thread
        ; mov curr_thd_data, reg1
        ;; Self::free_curr_thread(jit)
        ; end:
        );
    }

    fn return_result(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov rsi, [rbp + current_match_offset!()]
        ; test rsi, rsi
        ; jz >no_match
        ; add rsi, mem
        ; mov rdi, [rbp + result_offset!()]
        ; mov rcx, [rbp + result_len_offset!()]
        // This assumes that result_len <= array_len, which is checked before calling this code
        // We could also take the minimum between the two, but
        ; rep movsq
        ; return_:
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
        Self::free_list_size(jit)
            // +1 for the current match
            + Self::array_size(jit) * (jit.max_concurrent_threads() + 1)
    }

    fn initialize_cg_region(jit: &mut PikeJIT) {
        let free_list_start = jit.cg_mem_start();
        let array_start = (free_list_start + Self::free_list_size(jit)) as u32;
        __!(jit.ops,
          mov QWORD [rbp + current_match_offset!()], 0
        ; lea rsp, [rbp + current_match_offset!()]
        ; lea cg_reg, [mem + (jit.cg_mem_start() as i32)]
        ; mov QWORD [cg_reg], array_start.cast_signed()
        )
    }

    fn alloc_thread(jit: &mut PikeJIT) {
        __!(jit.ops, call ->alloc_empty_array);
    }

    // Must not use end as a local label
    fn free_curr_thread(jit: &mut PikeJIT) {
        let offset = (jit.register_count * ptr_size!()) as i32;
        __!(jit.ops,
          dec QWORD [mem + curr_thd_data + offset]
        ; mov reg1, [mem + curr_thd_data + offset]
        ; test reg1, reg1
        ; jnz >next
        ; add cg_reg, ptr_size!()
        ; mov QWORD [cg_reg], curr_thd_data
        ; next:
        )
    }

    fn clone_curr_thread(jit: &mut PikeJIT) {
        let offset = (jit.register_count * ptr_size!()) as i32;
        __!(jit.ops, inc[mem + curr_thd_data + offset]);
    }

    fn at_code_end(jit: &mut PikeJIT) {
        __!(jit.ops,
          ->alloc_empty_array:
        ; lea curr_thd_data, [mem + (jit.cg_mem_start() as i32)]
        ; cmp curr_thd_data, cg_reg
        ; je >empty_case
        ; mov curr_thd_data, [cg_reg]
        ; sub cg_reg, ptr_size!()
        ; jmp >set_all_to_invalid
        ; empty_case:
        ; mov reg1, [cg_reg]
        ; mov curr_thd_data, reg1
        ; add reg1, ((Self::array_size(jit) * ptr_size!()) as u32).cast_signed()
        ; mov [cg_reg], reg1
        ; set_all_to_invalid:
        ;; {
        for i in 0..jit.register_count {
            let offset = i * ptr_size!();
            if i % 2 == 0 {
                __!(jit.ops, mov QWORD [mem + curr_thd_data + offset as i32], 1);
            } else {
                __!(jit.ops, mov QWORD [mem + curr_thd_data + offset as i32], 0);
            }
        }
        }
        // Set ref-count to 1
        ; mov QWORD [mem + curr_thd_data + (jit.register_count * ptr_size!()) as i32], 1
        ; ret

        ; ->clone_array:
        ; lea reg1, [mem + (jit.cg_mem_start() as i32)]
        ; cmp reg1, cg_reg
        ; je >empty_case
        ; mov reg1, [cg_reg]
        ; sub cg_reg, ptr_size!()
        ; jmp >array_copy
        ; empty_case:
        ; mov reg1, [cg_reg]
        ; mov reg2, reg1
        ; add reg2, ((Self::array_size(jit) * ptr_size!()) as u32).cast_signed()
        ; mov [cg_reg], reg2
        ; array_copy:
        // TODO: using rep movs did not seem to improve performance
        ;; {
        for i in 0..jit.register_count {
            let offset = (i * ptr_size!()) as i32;
            __!(jit.ops,
              mov reg2, [mem + curr_thd_data + offset]
            ; mov [mem + reg1 + offset], reg2);
        }
        }
        ; mov curr_thd_data, reg1
        // Set ref-count to 1
        ; mov QWORD [mem + curr_thd_data + (jit.register_count * ptr_size!()) as i32], 1
        ; ret
        )
    }

    fn at_fetch_next_char(_: &mut PikeJIT) {
        // Nothing to do
    }

    fn require_thread_tree() -> bool {
        true
    }
}
