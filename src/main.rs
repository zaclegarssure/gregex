use std::io::{self, Write};

use gregex::Regex;
use gregex::pike_bytecode::Compiler;
use gregex::pike_jit::PikeJIT;
use gregex::pike_jit::cg_impl_array::CGImplArray;
use gregex::pike_jit::cg_impl_register::CGImplReg;
use regex_syntax::Parser;

fn main() {
    println!("Gregex REPL");
    println!("Type an empty pattern to exit.");

    loop {
        print!("regex> ");
        io::stdout().flush().unwrap();
        let mut pattern = String::new();
        if io::stdin().read_line(&mut pattern).is_err() {
            println!("Error reading pattern.");
            continue;
        }
        let pattern = pattern.trim();
        if pattern.is_empty() {
            break;
        }

        let hir = match Parser::new().parse(pattern) {
            Ok(hir) => hir,
            Err(e) => {
                println!("Regex parse error: {e}");
                continue;
            }
        };

        let capture_count = hir.properties().explicit_captures_len();
        let register_count = 2 * (capture_count + 1);
        let bytecode = match Compiler::compile(hir) {
            Ok(bc) => bc,
            Err(e) => {
                println!("Compile error: {e:?}");
                continue;
            }
        };

        let jitted = if capture_count > 0 {
            match PikeJIT::compile::<CGImplArray>(&bytecode, register_count) {
                Ok(jitted) => jitted,
                Err(e) => {
                    println!("Jit error: {e:?}");
                    continue;
                }
            }
        } else {
            match PikeJIT::compile::<CGImplReg>(&bytecode, register_count) {
                Ok(jitted) => jitted,
                Err(e) => {
                    println!("Jit error: {e:?}");
                    continue;
                }
            }
        };

        loop {
            println!("Type exit to go back to the regex prompt.");
            print!("input> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                println!("Error reading input.");
                continue;
            }
            let input = input.trim();
            if input == "exit" {
                break;
            }
            match jitted.find(input) {
                Some(m) => {
                    println!("Matched: {}", m.slice(),);
                }
                None => println!("No match."),
            }
        }
    }
}
