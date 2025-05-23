use crate::pike_jit::PikeJIT;
use dynasmrt::{DynasmApi, DynasmLabelApi, dynasm};

use super::cg_implementation::CGImpl;

/// CG implementation when no capture groups are present.
/// This encode
pub struct CGImplReg;

// Since there are no capture group, the match fits on 2 usize,
// which on x64 is 2*64bits.
cst!(
    current_match_offset,
    last_saved_value_offset!() - (2 * ptr_size!())
);

impl CGImpl for CGImplReg {
    fn init_mem_size(_: &PikeJIT) -> usize {
        0
    }

    fn return_result(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov rax, [rbp + current_match_offset!()]
        ; mov cg_reg, [rbp + current_match_offset!() + ptr_size!()]
        ; cmp cg_reg, rax
        ; jb >no_match
        ; mov rcx, [rbp + (result_offset!())]
        ; mov QWORD [rcx], rax
        ; mov QWORD [rcx+ ptr_size!()], cg_reg
        ; mov rax, 1
        ;; jit.epilogue()
        ; ret
        ; no_match:
        ; mov rax, 0
        ;; jit.epilogue()
        ; ret
        )
    }

    fn write_reg(jit: &mut PikeJIT, reg: u32) {
        // The closing group operation is done at accept time
        assert!(reg == 0);
        __!(jit.ops, mov curr_thd_data, input_pos);
    }

    fn accept_curr_thread(jit: &mut PikeJIT) {
        __!(jit.ops,
          mov [rbp + current_match_offset!()], curr_thd_data
        ; mov [rbp + current_match_offset!() + ptr_size!()], input_pos
        )
    }

    fn initialize_cg_region(jit: &mut PikeJIT) {
        // To represent an invalid match we set match_end < match_begin
        __!(jit.ops,
          mov QWORD [rbp + current_match_offset!()], 1
        ; mov QWORD [rbp + current_match_offset!() + ptr_size!()], 0
        ; lea rsp, [rbp + current_match_offset!()]
        )
    }

    fn alloc_thread(_: &mut PikeJIT) {
        // Nothing to do
    }

    fn free_curr_thread(_: &mut PikeJIT) {
        // Nothing to do
    }

    fn clone_curr_thread(_: &mut PikeJIT) {
        // Nothing to do
    }

    fn at_code_end(_: &mut PikeJIT) {
        // Nothing to do
    }

    fn at_fetch_next_char(_: &mut PikeJIT) {
        // Nothing to do
    }
}
