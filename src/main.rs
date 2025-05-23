use std::io::{self, Write};

use regex_jit_prototype::pike_bytecode::Compiler;
use regex_jit_prototype::pike_jit::PikeJIT;
use regex_jit_prototype::pike_jit::cg_impl_array::CGImplArray;
use regex_jit_prototype::pike_jit::cg_impl_register::CGImplReg;
use regex_jit_prototype::pike_vm::PikeVM;
use regex_syntax::Parser;

fn main() {
    println!("PikeVM Regex REPL");
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

        //let vm = PikeVM::new(bytecode, capture_count);
        let jitted = match PikeJIT::compile::<CGImplArray>(&bytecode, register_count) {
            Ok(jitted) => jitted,
            Err(e) => {
                println!("Jit error: {e:?}");
                continue;
            }
        };

        loop {
            println!("Type return to go back to the regex prompt.");
            print!("input> ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                println!("Error reading input.");
                continue;
            }
            let input = input.trim();
            if input == "return" {
                break;
            }
            match jitted.exec(input) {
                Some(m) => {
                    println!("Matched!");
                    for i in 0..capture_count + 1 {
                        if let Some(s) = m.get(i) {
                            println!("Group {i}: {:?}", s);
                        } else {
                            println!("Group {i}: None");
                        }
                    }
                }
                None => println!("No match."),
            }
        }
    }
}
