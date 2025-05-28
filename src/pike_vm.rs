use std::collections::{HashSet, VecDeque};

use crate::{
    pike_bytecode::Instruction::{self, *},
    regex::{self, Regex},
    util::{Captures, Input, Match, Span},
};

pub struct PikeVM {
    bytecode: Vec<Instruction>,
    capture_count: usize,
}

#[derive(Clone, Debug)]
struct Thread {
    pc: usize,
    registers: Box<[Option<usize>]>,
}
// TODO Update this to correctly accoutn for overall match
// and the lazy star being implicit

impl Thread {
    fn new(register_count: usize, pc: usize) -> Self {
        let vec = vec![None; register_count];
        let registers = vec.into_boxed_slice();
        Self { pc, registers }
    }
    fn write_reg(&mut self, reg: u32, value: usize) {
        self.registers[reg as usize] = Some(value)
    }

    fn inc_pc(mut self) -> Self {
        self.pc += 1;
        self
    }

    fn with_pc(mut self, pc: usize) -> Self {
        self.pc = pc;
        self
    }

    fn into_match(self, subject: &str) -> Match<'_> {
        Match::new(
            subject,
            self.registers[0].unwrap()..self.registers[1].unwrap(),
        )
    }
}

struct State {
    active: VecDeque<Thread>,
    next: VecDeque<Thread>,
    input_pos: usize,
    visited: HashSet<usize>,
    best_match: Option<Thread>,
}

impl State {
    fn new(state_count: usize) -> Self {
        Self {
            active: VecDeque::with_capacity(state_count),
            next: VecDeque::with_capacity(state_count),
            // TODO Fix this later
            input_pos: 0,
            visited: HashSet::with_capacity(state_count),
            best_match: None,
        }
    }

    fn accept(&mut self, thread: Thread) {
        self.best_match = Some(thread);
        self.active.clear();
    }

    fn push_active(&mut self, thread: Thread) {
        self.active.push_front(thread);
    }

    fn pop_active(&mut self) -> Option<Thread> {
        self.active.pop_front()
    }

    fn pop_active_until_not_visited(&mut self) -> Option<Thread> {
        while let Some(thread) = self.pop_active() {
            if self.visited.insert(thread.pc) {
                return Some(thread);
            }
        }
        None
    }

    fn push_next(&mut self, thread: Thread) {
        self.next.push_back(thread);
    }

    fn swap_and_advance_by(&mut self, step: usize) {
        self.visited.clear();
        self.input_pos += step;
        std::mem::swap(&mut self.active, &mut self.next);
    }

    fn step_at_end(&mut self, bytecode: &[Instruction]) {
        let state = self;
        while let Some(mut thread) = state.pop_active_until_not_visited() {
            match &bytecode[thread.pc] {
                Fork2(a, b) => {
                    state.push_active(thread.clone().with_pc(*b));
                    state.push_active(thread.with_pc(*a));
                }
                ForkN(branches) => {
                    for pc in branches.iter().rev() {
                        state.push_active(thread.clone().with_pc(*pc));
                    }
                }
                Jmp(target) => {
                    state.push_active(thread.with_pc(*target));
                }
                WriteReg(r) => {
                    thread.write_reg(*r, state.input_pos);
                    state.push_active(thread.inc_pc());
                }
                Accept => {
                    state.accept(thread);
                    break;
                }
                _ => (),
            }
        }
    }

    fn step(&mut self, bytecode: &[Instruction], c: char) {
        let state = self;
        while let Some(mut thread) = state.pop_active_until_not_visited() {
            match &bytecode[thread.pc] {
                Consume(c2) if *c2 == c => {
                    state.push_next(thread.inc_pc());
                }
                ConsumeAny => {
                    state.push_next(thread.inc_pc());
                }
                Fork2(a, b) => {
                    state.push_active(thread.clone().with_pc(*b));
                    state.push_active(thread.with_pc(*a));
                }
                ForkN(branches) => {
                    for pc in branches.iter().rev() {
                        state.push_active(thread.clone().with_pc(*pc));
                    }
                }
                ConsumeClass(class) => {
                    for (start, end) in class.iter() {
                        if c < *start {
                            break;
                        } else if c > *end {
                            continue;
                        }
                        state.push_next(thread.inc_pc());
                        break;
                    }
                }
                Jmp(target) => {
                    state.push_active(thread.with_pc(*target));
                }
                WriteReg(r) => {
                    thread.write_reg(*r, state.input_pos);
                    state.push_active(thread.inc_pc());
                }
                Accept => {
                    state.accept(thread);
                    break;
                }
                _ => (),
            }
        }
    }
}

impl PikeVM {
    pub fn new(bytecode: Vec<Instruction>, capture_count: usize) -> Self {
        Self {
            bytecode,
            capture_count,
        }
    }
}

impl Regex for PikeVM {
    fn find<'s>(&self, input: impl Into<Input<'s>>) -> Option<Match<'s>> {
        let Input {
            subject,
            span: Span { mut from, to },
            anchored,
            first_match,
        } = input.into();
        let state_count = self.bytecode.len();
        let mut state = State::new(state_count);
        let register_count = (self.capture_count + 1) * 2;
        state.push_active(Thread::new(register_count, from));
        for c in subject[from..to].chars() {
            state.step(&self.bytecode, c);
            if !state.next.is_empty() {
                match state.best_match {
                    Some(best_match) if first_match => {
                        return Some(best_match.into_match(subject));
                    }
                    None if !anchored => {
                        from += c.len_utf8();
                        state.push_next(Thread::new(register_count, from));
                    }
                    _ => (),
                };
                state.swap_and_advance_by(c.len_utf8());
            } else {
                return state.best_match.map(|t| t.into_match(subject));
            }
        }
        state.step_at_end(&self.bytecode);
        state.best_match.map(|t| t.into_match(subject))
    }

    fn find_all<'s>(&self, input: impl Into<Input<'s>>) -> impl Iterator<Item = Match<'s>> {
        regex::FindAll::new(self, input.into())
    }

    fn find_captures<'s>(&self, input: impl Into<Input<'s>>) -> Option<Captures<'s>> {
        todo!()
    }

    fn find_all_captures<'s>(
        &self,
        input: impl Into<Input<'s>>,
    ) -> impl Iterator<Item = Captures<'s>> {
        regex::FindAllCaptures::new(self, input.into())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use regex_syntax::Parser;

//     fn compile_and_exec<'s>(pattern: &str, input: &'s str) -> Option<Match<'s>> {
//         let hir = Parser::new().parse(pattern).unwrap();
//         let capture_count = hir.properties().explicit_captures_len();
//         let bytecode = crate::pike_bytecode::Compiler::compile(hir).unwrap();
//         let vm = PikeVM::new(bytecode, capture_count);
//         vm.exec(input)
//     }

//     #[test]
//     fn test_literal_match() {
//         let m = compile_and_exec("abc", "abc");
//         assert!(m.is_some());
//         assert_eq!(&m.unwrap()[0], "abc");
//     }

//     #[test]
//     fn test_literal_no_match() {
//         let m = compile_and_exec("abc", "def");
//         assert!(m.is_none());
//     }

//     #[test]
//     fn test_alternation() {
//         assert!(compile_and_exec("foo|bar", "foo").is_some());
//         assert!(compile_and_exec("foo|bar", "bar").is_some());
//         assert!(compile_and_exec("foo|bar", "baz").is_none());
//     }

//     #[test]
//     fn test_repetition() {
//         assert!(compile_and_exec("a*", "").is_some());
//         assert!(compile_and_exec("a*", "a").is_some());
//         assert!(compile_and_exec("a*", "aaaa").is_some());
//         assert!(compile_and_exec("a*", "b").is_some()); // matches empty prefix
//     }

//     #[test]
//     fn test_class() {
//         assert!(compile_and_exec("[a-z]", "a").is_some());
//         assert!(compile_and_exec("[a-z]", "m").is_some());
//         assert!(compile_and_exec("[a-z]", "A").is_none());
//     }

//     #[test]
//     fn test_capture_group() {
//         let m = compile_and_exec("(hi)", "hi");
//         assert!(m.is_some());
//         let m = m.unwrap();
//         // Group 1 should capture "hi"
//         assert_eq!(m.get(1), Some("hi"));
//     }
// }
