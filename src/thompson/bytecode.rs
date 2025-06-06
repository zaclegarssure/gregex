//! A Thompson NFA represented in a bytecode format.
//!
//! This module contains the definition of [`Instruction`], a
//! bytecode format that represents a Thompson's NFA, which is
//! one possible NFA representation of a regular expression whose
//! particularities is to be linearly proportional in size to the
//! pattern. Furthermore compiling a pattern to this representation
//! take linear time. The compiler is also provided by this module,
//! see [`Compiler`].
use std::{collections::HashMap, error::Error, fmt};

use crate::{regex::Config, util::Char};

/// Bytecode
#[derive(Debug, Clone)]
pub enum Instruction {
    Consume(Char),
    ConsumeAny,
    ConsumeClass(Box<[(Char, Char)]>),
    /// Consume a class which was outlined during the compilation process
    /// This is done with large classes to reduce memory usage for the interpreter
    /// and make the jitted smaller (and therefore more cache-friendly), since
    /// there classes are inlined.
    ConsumeOutlined(usize),
    Fork2(usize, usize),
    ForkN(Box<[usize]>),
    Jmp(usize),
    WriteReg(u32),
    Assertion(Look),
    Accept,
}

use Instruction::*;
use regex_syntax::hir::{Capture, Class, Hir, HirKind, Literal, Look, Repetition};

/// Compilation error
/// TODO: Explain why each of these senario can occure
#[derive(Debug)]
pub enum CompileError {
    InvalidUtf8,
    ContainsLookAround,
    ContainsNamedCaptureGroup,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::InvalidUtf8 => write!(f, "Pattern contains non-unicode sequence"),
            CompileError::ContainsLookAround => write!(f, "Pattern contains look-around"),
            CompileError::ContainsNamedCaptureGroup => {
                write!(f, "Pattern contains named capture groups")
            }
        }
    }
}

impl Error for CompileError {}

#[derive(Debug, Default)]
pub struct Bytecode {
    // TODO: Make these fields private, and only alow reading them most likely
    pub instructions: Vec<Instruction>,
    pub barriers: Vec<bool>,
    pub outlined_classes: Vec<Box<[(Char, Char)]>>,
}

/// A compiler from [`regex_syntax::hir::Hir`] to
/// this bytecode representation.
#[derive(Debug, Default)]
pub struct Compiler {
    bytecode: Bytecode,
    outlined_classes: HashMap<Box<[(Char, Char)]>, usize>,
    config: Config,
}

impl Compiler {
    /// Try to compile a regex in [`regex_syntax::hir::Hir`] form to
    /// this bytecode.
    pub fn compile(hir: Hir, config: Config) -> Result<Bytecode, CompileError> {
        if !hir.properties().is_utf8() {
            return Err(CompileError::InvalidUtf8);
        }
        let mut compiler = Compiler {
            config,
            ..Default::default()
        };
        compiler.compile_internal(hir, false);
        compiler.push(Accept, false);
        Ok(compiler.bytecode)
    }

    fn current_pc(&self) -> usize {
        self.bytecode.instructions.len()
    }

    fn push(&mut self, instruction: Instruction, barrier: bool) {
        self.bytecode.instructions.push(instruction);
        self.bytecode.barriers.push(barrier);
    }

    fn fork2(a: usize, b: usize, greedy: bool) -> Instruction {
        if greedy { Fork2(a, b) } else { Fork2(b, a) }
    }

    /// Compiles the given hir to Bytecode.
    /// Takes as parameter whenever a barrier should be added for the first instruction
    /// in the compiled code, and returns whenever whatever comes after should have a barrier.
    fn compile_internal(&mut self, hir: Hir, mut barrier: bool) -> bool {
        match hir.into_kind() {
            HirKind::Empty => barrier,
            HirKind::Literal(Literal(bytes)) => {
                // Ok because we check for Hir::is_utf8() before
                let string = str::from_utf8(&bytes).unwrap();
                // We could also directly decode the chars from the bytes
                // without creating the &str.
                for c in string.chars() {
                    self.push(Consume(c.into()), barrier);
                    if barrier {
                        // Only the first consume need a barrier (if it was required in the first place)
                        barrier = false;
                    }
                }
                barrier
            }
            HirKind::Class(class) => {
                let class = match class {
                    Class::Unicode(class_unicode) => class_unicode
                        .iter()
                        .map(|c| (c.start().into(), c.end().into()))
                        .collect::<Box<[_]>>(),
                    Class::Bytes(class_byte) => class_byte
                        .iter()
                        .map(|c| (c.start().into(), c.end().into()))
                        .collect::<Box<[_]>>(),
                };
                // TODO: Parametrized this
                if class.len() > 4 {
                    let id = match self.outlined_classes.get(&class) {
                        Some(id) => *id,
                        None => {
                            let id = self.bytecode.outlined_classes.len();
                            // TODO: Find a way to avoid this cloning
                            self.outlined_classes.insert(class.clone(), id);
                            self.bytecode.outlined_classes.push(class);
                            id
                        }
                    };
                    self.push(ConsumeOutlined(id), barrier);
                } else {
                    self.push(ConsumeClass(class), barrier);
                }
                false
            }
            HirKind::Look(look) => {
                self.push(Assertion(look), barrier);
                false
            }
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
                    // Same as Literal and Concat, only the begining may require a barrier
                    barrier = self.compile_internal(*sub.clone(), barrier);
                }
                match max {
                    Some(max) => {
                        let diff = (max - min) as usize;
                        let mut forks_pc = Vec::with_capacity(diff);
                        for _ in min..max {
                            forks_pc.push(self.current_pc());
                            self.push(Fork2(0, 0), barrier);
                            barrier = self.compile_internal(*sub.clone(), false);
                        }
                        let end_pc = self.current_pc();
                        for fork_pc in forks_pc {
                            self.bytecode.instructions[fork_pc] =
                                Self::fork2(fork_pc + 1, end_pc, greedy);
                        }
                        // TODO: There are some rare cases where this is not necessary
                        true
                    }
                    None => match last_iter_start {
                        Some(last_iter_start) => {
                            self.push(
                                Self::fork2(last_iter_start, self.current_pc() + 1, greedy),
                                barrier,
                            );
                            self.bytecode.barriers[last_iter_start] = true;
                            false
                        }
                        None => {
                            let fork_pc = self.current_pc();
                            self.push(Fork2(0, 0), true);
                            barrier = self.compile_internal(*sub, false);
                            // Technnically we could pass false here, since it will immediatly jump to
                            // an instruction (the first fork) with a barrier
                            self.push(Jmp(fork_pc), barrier);
                            self.bytecode.instructions[fork_pc] =
                                Self::fork2(fork_pc + 1, self.current_pc(), greedy);
                            false
                        }
                    },
                }
            }
            HirKind::Capture(Capture { index, name, sub }) => {
                if self.config.cg {
                    // TODO: Add support for this
                    assert!(name.is_none());
                    self.push(WriteReg(index * 2), barrier);
                    let barrier = self.compile_internal(*sub, false);
                    self.push(WriteReg(index * 2 + 1), barrier);
                    false
                } else {
                    self.compile_internal(*sub, barrier)
                }
            }
            HirKind::Concat(hirs) => {
                for hir in hirs {
                    barrier = self.compile_internal(hir, barrier);
                }
                barrier
            }
            // Quick annoying fun fact for anyone who would read this:
            // In regex-syntax (rust regex) Alternation means a regex of the form e1|e2|e3,
            // and concatenation is e1e2e3. In V8 (and I guess JS in general) alternation
            // means e1e2e3 and disjuction means e1|e2|e3.
            HirKind::Alternation(hirs) => {
                let length = hirs.len();
                let mut fork_targets = Vec::with_capacity(length);
                let mut jmps = Vec::with_capacity(length - 1);
                let current_pc = self.current_pc();
                // Just to allocate some space
                self.push(ConsumeAny, barrier);
                for (i, hir) in hirs.into_iter().enumerate() {
                    fork_targets.push(self.current_pc());
                    let barrier = self.compile_internal(hir, false);
                    if i < length - 1 {
                        jmps.push(self.current_pc());
                        // Patched just below
                        self.push(Jmp(0), barrier);
                    }
                }
                self.bytecode.instructions[current_pc] = ForkN(fork_targets.into_boxed_slice());
                // Path jumps to point to the end of the alternation
                for pc in jmps {
                    self.bytecode.instructions[pc] = Jmp(self.current_pc())
                }
                true
            }
        }
    }
}
