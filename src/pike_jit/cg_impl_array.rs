use dynasmrt::{DynasmApi, DynasmLabelApi, dynasm};

use super::{PikeJIT, cg_implementation::CGImpl};

pub struct CGImplArray;

impl CGImplArray {
    fn free_list_size(jit: &PikeJIT) -> usize {
        (jit.max_concurrent_threads() + 2) * ptr_size!()
    }

    fn array_size(jit: &PikeJIT) -> usize {
        jit.register_count * ptr_size!()
    }

    fn free_all_threads_in_active(jit: &mut PikeJIT) {
        __!(jit.ops,
          loop_:
        ;; jit.pop_active()
        ; test curr_thd_data, curr_thd_data
        // The sentinel thread (the one switching to the next iteration)
        // has 0 as it's thread data.
        ; jz >next
        ;; Self::free_curr_thread(jit)
        ; jmp <loop_
        ; next:
        )
    }
}

// The match is only saved as a pointer to the array
// containing it. Therefore it's 64 bits long.
cst!(
    current_match_offset,
    last_saved_value_offset!() - (ptr_size!())
);

impl CGImpl for CGImplArray {
    fn write_reg(jit: &mut PikeJIT, reg: u32) {
        // TODO, again make sure we avoid having overflow on indices
        __!(jit.ops,
          mov QWORD [mem + curr_thd_data + ((reg*ptr_size!()) as i32)], input_pos
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
        ; jz >next
        // TODO Find a way to pass a register to free_curr_thread
        ; mov curr_thd_data, reg1
        ;; Self::free_curr_thread(jit)
        ; next:
        );
    }

    fn return_result(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov curr_thd_data, [rbp + current_match_offset!()]
        ; test curr_thd_data, curr_thd_data
        ; jz >no_match
        ; mov reg2, [rbp + result_offset!()]
        ; add curr_thd_data, mem
        ; mov input_len, [rbp + result_len_offset!()]
        ;; {
        // We always unroll this loop, but maybe this should depend
        // on the number of capture groups.
        // We could also call memcopy.
        for i in 0..jit.register_count {
            // TODO: As always fix this offsets
            let offset = (i*ptr_size!()) as i32;
            if i == 1 {
                __!(jit.ops,
                 // This is result_len
                  cmp input_pos, offset
                ; jbe >return_
                ; mov [reg2 + offset], input_pos
                )
            } else {
                __!(jit.ops,
                 // This is result_len
                  cmp input_pos, offset
                ; jbe >return_
                ; mov reg1, [curr_thd_data + offset]
                ; mov [reg2 + offset], reg1
                )
            }
        }
        }
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
        CGImplArray::free_list_size(jit)
            + CGImplArray::array_size(jit) * jit.max_concurrent_threads()
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
        __!(jit.ops, call ->alloc_empty_array)
    }

    fn free_curr_thread(jit: &mut PikeJIT) {
        __!(jit.ops,
          add cg_reg, ptr_size!()
        ; mov QWORD [cg_reg], curr_thd_data
        )
    }

    fn clone_curr_thread(jit: &mut PikeJIT) {
        __!(jit.ops, call ->clone_array)
    }

    fn at_code_end(jit: &mut PikeJIT) {
        __!(jit.ops,
          ->alloc_empty_array:
        ; lea curr_thd_data, [mem + (jit.cg_mem_start() as i32)]
        ; cmp curr_thd_data, cg_reg
        ; je >empty_case
        ; mov curr_thd_data, [cg_reg]
        ; sub cg_reg, ptr_size!()
        ; jmp >set_all_to_minus_1
        ; empty_case:
        ; mov reg1, [cg_reg]
        ; mov curr_thd_data, reg1
        ; add reg1, (Self::array_size(jit) as u32).cast_signed()
        ; mov [cg_reg], reg1
        ; set_all_to_minus_1:
        ; xor reg1, reg1
        ; dec reg1
        ;; {
        for i in 0..jit.register_count {
            let offset = i * ptr_size!();
            __!(jit.ops, mov [mem + curr_thd_data + offset as i32], reg1);
        }
        }
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
        ; add reg2, (Self::array_size(jit) as u32).cast_signed()
        ; mov [cg_reg], reg2
        ; array_copy:
        ;; {
        for i in 0..jit.register_count {
            let offset = (i * ptr_size!()) as i32;
            __!(jit.ops,
              mov reg2, [mem + curr_thd_data + offset]
            ; mov [mem + reg1 + offset], reg2);
        }
        }
        ; mov curr_thd_data, reg1
        ; ret
        )
    }

    fn at_fetch_next_char(_: &mut PikeJIT) {
        // Nothing to do
    }
}
