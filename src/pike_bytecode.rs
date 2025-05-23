use core::str;

use regex_syntax::hir::{Capture, Class, Hir, HirKind, Literal, Repetition};

#[derive(Debug)]
pub enum Instruction {
    Consume(char),
    ConsumeAny,
    ConsumeClass(Class),
    Fork2(usize, usize),
    ForkN(Vec<usize>),
    Jmp(usize),
    WriteReg(u32),
    Accept,
}

use Instruction::*;

#[derive(Debug)]
pub enum CompileError {
    InvalidUtf8,
    ContainsLookAround,
    ContainsNamedCaptureGroup,
}

type Bytecode = Vec<Instruction>;

pub struct Compiler {
    bytecode: Bytecode,
}

impl Compiler {
    pub fn compile(hir: Hir) -> Result<Bytecode, CompileError> {
        if !hir.properties().is_utf8() {
            return Err(CompileError::InvalidUtf8);
        }
        if !hir.properties().look_set().is_empty() {
            return Err(CompileError::ContainsLookAround);
        }
        let mut compiler = Compiler {
            bytecode: Vec::new(),
        };
        compiler.push_lazy_star();
        compiler.push(WriteReg(0));
        compiler.compile_internal(hir);
        // Write reg 1 is done implicitely in Accept
        compiler.push(Accept);
        Ok(compiler.bytecode)
    }

    fn push_lazy_star(&mut self) {
        self.push(Fork2(3, 1));
        self.push(ConsumeAny);
        self.push(Jmp(0));
    }

    fn current_pc(&self) -> usize {
        self.bytecode.len()
    }

    fn push(&mut self, instruction: Instruction) {
        self.bytecode.push(instruction);
    }

    fn fork2(a: usize, b: usize, greedy: bool) -> Instruction {
        if greedy { Fork2(a, b) } else { Fork2(b, a) }
    }

    fn compile_internal(&mut self, hir: Hir) {
        match hir.into_kind() {
            HirKind::Empty => (),
            HirKind::Literal(Literal(bytes)) => {
                // Ok because we check for Hir::is_utf8() before
                let string = str::from_utf8(&bytes).unwrap();
                for c in string.chars() {
                    self.push(Consume(c));
                }
            }
            HirKind::Class(class) => {
                self.push(ConsumeClass(class));
            }
            HirKind::Look(_) => unreachable!(),
            HirKind::Repetition(Repetition {
                min,
                max,
                greedy,
                sub,
            }) => {
                let mut last_iter_start = None;
                for i in 0..min {
                    if i == min - 1 {
                        last_iter_start = Some(self.current_pc());
                    }
                    self.compile_internal(*sub.clone());
                }
                match max {
                    Some(max) => {
                        let diff = (max - min) as usize;
                        let mut forks_pc = Vec::with_capacity(diff);
                        for _ in min..max {
                            forks_pc.push(self.current_pc());
                            self.push(Fork2(0, 0));
                            self.compile_internal(*sub.clone());
                        }
                        let end_pc = self.current_pc();
                        for fork_pc in forks_pc {
                            self.bytecode[fork_pc] = Self::fork2(fork_pc + 1, end_pc, greedy);
                        }
                    }
                    None => match last_iter_start {
                        Some(last_iter_start) => {
                            self.push(Self::fork2(last_iter_start, self.current_pc() + 1, greedy));
                        }
                        None => {
                            let fork_pc = self.current_pc();
                            self.push(Fork2(0, 0));
                            self.compile_internal(*sub);
                            self.push(Jmp(fork_pc));
                            self.bytecode[fork_pc] =
                                Self::fork2(fork_pc + 1, self.current_pc(), greedy);
                        }
                    },
                }
            }
            HirKind::Capture(Capture { index, name, sub }) => {
                // TODO: Check this before
                assert!(name.is_none());
                self.push(WriteReg(index * 2));
                self.compile_internal(*sub);
                self.push(WriteReg(index * 2 + 1));
            }
            HirKind::Concat(hirs) => {
                for hir in hirs {
                    self.compile_internal(hir);
                }
            }
            HirKind::Alternation(hirs) => {
                let length = hirs.len();
                let mut fork_targets = Vec::with_capacity(length);
                let mut jmps = Vec::with_capacity(length - 1);
                let current_pc = self.current_pc();
                self.push(ForkN(Vec::new()));
                for (i, hir) in hirs.into_iter().enumerate() {
                    fork_targets.push(self.current_pc());
                    self.compile_internal(hir);
                    if i < length - 1 {
                        jmps.push(self.current_pc());
                        self.push(Jmp(0));
                    }
                }
                self.bytecode[current_pc] = ForkN(fork_targets);
                for pc in jmps {
                    self.bytecode[pc] = Jmp(self.current_pc())
                }
            }
        }
    }
}
