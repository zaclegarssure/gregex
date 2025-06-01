//! An interpreter for [`crate::thompson::bytecode`].

use std::{
    cmp::min,
    collections::{HashSet, VecDeque},
    error::Error,
};

use regex_syntax::Parser;

use crate::{
    regex::{Config, RegexImpl},
    thompson::bytecode::{
        Compiler,
        Instruction::{self, *},
    },
    util::{Char, Input, Match, Span},
};

/// A so-called PikeVM.
///
/// This is an interpreter for the bytecode.
pub struct PikeVM {
    bytecode: Vec<Instruction>,
    capture_count: usize,
}

/// A thread currently alive in the bytecode.
#[derive(Clone, Debug)]
struct Thread {
    pc: usize,
    captures: Box<[Span]>,
}

impl Thread {
    fn new(capture_count: usize, pc: usize) -> Self {
        let vec = vec![Span::invalid(); capture_count];
        let captures = vec.into_boxed_slice();
        Self { pc, captures }
    }
    fn write_reg(&mut self, reg: usize, value: usize) {
        if reg % 2 == 0 {
            self.captures[reg / 2].from = value;
        } else {
            self.captures[reg / 2].to = value;
        }
    }

    fn inc_pc(mut self) -> Self {
        self.pc += 1;
        self
    }

    fn with_pc(mut self, pc: usize) -> Self {
        self.pc = pc;
        self
    }
}

pub struct State {
    active: VecDeque<Thread>,
    next: VecDeque<Thread>,
    input_pos: usize,
    visited: HashSet<usize>,
    best_match: Option<Thread>,
}

impl State {
    fn new(state_count: usize, input_pos: usize) -> Self {
        Self {
            active: VecDeque::with_capacity(state_count),
            next: VecDeque::with_capacity(state_count),
            input_pos,
            visited: HashSet::with_capacity(state_count),
            best_match: None,
        }
    }

    fn accept(&mut self, mut thread: Thread) {
        thread.write_reg(1, self.input_pos);
        self.best_match = Some(thread);
        self.active.clear();
    }

    fn push_active(&mut self, thread: Thread) {
        self.active.push_front(thread);
    }

    fn pop_active(&mut self) -> Option<Thread> {
        self.active.pop_front()
    }

    /// Pop the active queue until a thread whose pc was not visited already
    /// during this iteration is found, and returns it.
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

    fn reset(&mut self) {
        self.best_match = None;
        self.visited.clear();
        self.active.clear();
        self.next.clear();
    }
}

impl PikeVM {
    pub fn from_bytecode(bytecode: Vec<Instruction>, capture_count: usize) -> Self {
        Self {
            bytecode,
            capture_count,
        }
    }

    pub fn new(
        pattern: &str,
        config: Config,
    ) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let hir = Parser::from(config).parse(pattern)?;
        let capture_count = hir.properties().explicit_captures_len() + 1;
        let bytecode = Compiler::compile(hir)?;

        Ok(Self {
            bytecode,
            capture_count,
        })
    }

    fn step(&self, state: &mut State, c: Char) {
        let bytecode = self.bytecode.as_slice();
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
                    thread.write_reg(*r as usize, state.input_pos);
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

    pub fn capture_count(&self) -> usize {
        self.capture_count
    }
}

impl RegexImpl for PikeVM {
    type State = State;

    fn new_state(&self) -> Self::State {
        State::new(self.bytecode.len(), 0)
    }

    fn reset_state(&self, state: &mut Self::State) {
        state.reset();
    }

    fn find<'s>(&self, input: Input<'s>, state: &mut Self::State) -> Option<Match<'s>> {
        let subject = input.subject;
        let mut captures = vec![Span::invalid(); self.capture_count()].into_boxed_slice();
        if !self.find_captures(input, state, &mut captures) {
            return None;
        }

        Some(Match {
            subject,
            span: captures[0],
        })
    }

    fn find_captures<'s>(
        &self,
        input: Input<'s>,
        state: &mut Self::State,
        captures: &mut [Span],
    ) -> bool {
        if !input.valid() {
            return false;
        }

        let result_len = min(captures.len(), self.capture_count());

        let Input {
            subject,
            span: Span { from, to },
            anchored,
            first_match,
        } = input;

        state.input_pos = from;
        let register_count = self.capture_count();
        let mut first_thread = Thread::new(register_count, 0);
        first_thread.write_reg(0, from);
        state.push_active(first_thread);
        for c in subject[from..to].chars() {
            self.step(state, c.into());
            match &state.best_match {
                Some(best_match) if first_match || state.next.is_empty() => {
                    captures[0..result_len].copy_from_slice(&best_match.captures[0..result_len]);
                    return true;
                }
                Some(_) => {
                    state.swap_and_advance_by(c.len_utf8());
                }
                None if !anchored => {
                    let mut thread = Thread::new(register_count, 0);
                    thread.write_reg(0, state.input_pos + c.len_utf8());
                    state.push_next(thread);
                    state.swap_and_advance_by(c.len_utf8());
                }
                None => {
                    return false;
                }
            }
        }
        if to == subject.len() {
            self.step(state, Char::INPUT_BOUND);
        } else {
            // TODO: Find a nicer way to do this
            let c = subject[to..subject.len()].chars().next().unwrap().into();
            self.step(state, c);
        }
        // TODO avoid copy paste
        match &state.best_match {
            Some(best_match) => {
                captures[0..result_len].copy_from_slice(&best_match.captures[0..result_len]);
                true
            }
            None => false,
        }
    }
}
