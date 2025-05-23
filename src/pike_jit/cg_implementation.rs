use crate::pike_jit::PikeJIT;

pub trait CGImpl {
    fn write_reg(jit: &mut PikeJIT, reg: u32);
    /// Called when generating the code for the accept instruction.
    /// Since the capture groups are what encode the result
    /// the CG implementation is responsible for storing that result,
    /// when a thread reaches the accept instruction.
    fn accept_curr_thread(jit: &mut PikeJIT);

    /// Write the result of the best-match found to the result array.
    /// Or, if no match were found, simply return 0.
    fn return_result(jit: &mut PikeJIT);

    /// The initial size in byte requested by this implementation
    fn init_mem_size(jit: &PikeJIT) -> usize;
    /// Called before execution, to initialize any datastructures
    fn initialize_cg_region(jit: &mut PikeJIT);

    fn alloc_thread(jit: &mut PikeJIT);
    fn free_curr_thread(jit: &mut PikeJIT);
    fn clone_curr_thread(jit: &mut PikeJIT);

    /// Called at the end of the generation of the jitted code, to let the implementation
    /// add any code (such as helper functions) needed there.
    fn at_code_end(jit: &mut PikeJIT);
    /// Called before generating the code for fetching the next character.
    /// This is mostly usefull for implementations that may require the memory
    /// to grow at runtime.
    fn at_fetch_next_char(jit: &mut PikeJIT);
}
