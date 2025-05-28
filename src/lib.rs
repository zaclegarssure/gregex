use std::fmt;

use pike_bytecode::Compiler;
use pike_jit::{PikeJIT, cg_impl_array::CGImplArray, cg_impl_register::CGImplReg};
use regex_syntax::Parser;

pub mod pike_bytecode;
pub mod pike_jit;
pub mod pike_vm;
pub mod regex;
pub mod util;

pub use pike_jit::JittedRegex;
pub use regex::Regex;

#[derive(Debug, Clone)]
pub struct CompileError(String);

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::error::Error for CompileError {}

pub fn compile(pattern: &str) -> Result<JittedRegex, CompileError> {
    let hir = match Parser::new().parse(pattern) {
        Ok(hir) => hir,
        Err(e) => {
            return Err(CompileError(format!("Regex parse error: {e}")));
        }
    };

    let capture_count = hir.properties().explicit_captures_len();
    let register_count = 2 * (capture_count + 1);
    let bytecode = match Compiler::compile(hir) {
        Ok(bc) => bc,
        Err(e) => {
            return Err(CompileError(format!("Compile error: {e:?}")));
        }
    };

    let jitted = if capture_count > 0 {
        match PikeJIT::compile::<CGImplArray>(&bytecode, register_count) {
            Ok(jitted) => jitted,
            Err(e) => {
                return Err(CompileError(format!("Jit error: {e:?}")));
            }
        }
    } else {
        match PikeJIT::compile::<CGImplReg>(&bytecode, register_count) {
            Ok(jitted) => jitted,
            Err(e) => {
                return Err(CompileError(format!("Jit error: {e:?}")));
            }
        }
    };

    Ok(jitted)
}
