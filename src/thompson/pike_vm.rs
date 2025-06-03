//! An interpreter for [`crate::thompson::bytecode`].

use std::{cmp::min, collections::VecDeque, error::Error, mem};

use regex_syntax::{Parser, hir::Look};

use crate::{
    regex::{Config, RegexImpl},
    thompson::bytecode::{Bytecode, Compiler, Instruction::*},
    util::{Char, Input, Match, Span, find_prev_char},
};

/// A so-called PikeVM.
///
/// This is an interpreter for the bytecode.
pub struct PikeVM {
    bytecode: Bytecode,
    capture_count: usize,
}

/// A thread currently alive in the bytecode.
#[derive(Debug)]
struct Thread {
    pc: usize,
    capture_offset: usize,
}

impl Thread {
    fn write_reg(&self, reg: usize, value: usize, state: &mut State) {
        let offset = self.capture_offset + reg / 2;
        if offset >= self.capture_offset + state.result_len {
            return;
        }
        if reg % 2 == 0 {
            state.cg_arrays[offset].from = value;
        } else {
            state.cg_arrays[offset].to = value;
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

    fn free(self, state: &mut State) {
        state.cg_free.push(self.capture_offset);
    }

    fn dup(&self, state: &mut State) -> Self {
        let pc = self.pc;
        let capture_offset = state.alloc_array();
        state.cg_arrays.copy_within(
            self.capture_offset..(self.capture_offset + state.result_len),
            capture_offset,
        );
        Thread { pc, capture_offset }
    }
}

pub struct State {
    active: VecDeque<Thread>,
    next: VecDeque<Thread>,
    input_pos: usize,
    visited: Box<[usize]>,
    cg_free: Vec<usize>,
    cg_arrays: Box<[Span]>,
    best_match: Option<Thread>,
    capture_count: usize,
    result_len: usize,
}

impl State {
    fn new(capture_count: usize, state_count: usize, input_pos: usize) -> Self {
        Self {
            active: VecDeque::with_capacity(2 * state_count),
            next: VecDeque::with_capacity(2 * state_count),
            input_pos,
            visited: vec![0; state_count].into_boxed_slice(),
            best_match: None,
            cg_free: vec![0],
            cg_arrays: vec![Span::invalid(); (state_count * 2 + 1) * capture_count]
                .into_boxed_slice(),
            capture_count,
            result_len: 0,
        }
    }

    fn new_thread(&mut self, pc: usize) -> Thread {
        let capture_offset = self.alloc_array();
        self.cg_arrays[capture_offset..(capture_offset + self.result_len)].fill(Span::invalid());
        Thread { pc, capture_offset }
    }

    fn alloc_array(&mut self) -> usize {
        if self.cg_free.len() == 1 {
            let capture_offset = self.cg_free[0];
            self.cg_free[0] = capture_offset + self.result_len;
            capture_offset
        } else {
            self.cg_free.pop().unwrap()
        }
    }

    fn accept(&mut self, thread: Thread) {
        thread.write_reg(1, self.input_pos, self);
        if let Some(prev) = self.best_match.replace(thread) {
            prev.free(self);
        }
        let active = mem::take(&mut self.active);
        for thread in active {
            thread.free(self);
        }
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
            let value = &mut self.visited[thread.pc];
            if *value <= self.input_pos {
                *value = self.input_pos + 1;
                return Some(thread);
            } else {
                thread.free(self);
            }
        }
        None
    }

    fn push_next(&mut self, thread: Thread) {
        self.next.push_back(thread);
    }

    fn swap_and_advance_by(&mut self, step: usize) {
        self.input_pos += step;
        std::mem::swap(&mut self.active, &mut self.next);
    }

    fn reset(&mut self) {
        self.best_match = None;
        self.visited.fill(0);
        self.active.clear();
        self.next.clear();
        self.cg_free.clear();
        self.cg_free.push(0);
        self.result_len = 0;
    }

    fn write_best_match(&mut self, result: &mut [Span]) {
        let winning_thread = self.best_match.take().unwrap();
        let result_len = min(self.capture_count, result.len());
        let bounds = winning_thread.capture_offset..(winning_thread.capture_offset + result_len);
        result[0..result_len].copy_from_slice(&self.cg_arrays[bounds]);
    }
}

impl PikeVM {
    pub fn from_bytecode(bytecode: Bytecode, capture_count: usize) -> Self {
        Self {
            bytecode,
            capture_count,
        }
    }

    pub fn new(
        pattern: &str,
        config: Config,
    ) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let hir = Parser::from(config.clone()).parse(pattern)?;
        let capture_count = if config.cg {
            hir.properties().explicit_captures_len() + 1
        } else {
            1
        };
        let bytecode = Compiler::compile(hir, config)?;

        Ok(Self {
            bytecode,
            capture_count,
        })
    }

    /// Do one step of simulation, meaning stepping through all threads in the
    /// active queue and simulating them until they either die, or successfully consumed
    /// a character.
    fn step(&self, state: &mut State, prev: Char, c: Char) {
        let bytecode = self.bytecode.instructions.as_slice();
        'next_active: while let Some(mut thread) = state.pop_active_until_not_visited() {
            loop {
                match &bytecode[thread.pc] {
                    Consume(c2) if *c2 == c => {
                        state.push_next(thread.inc_pc());
                        break;
                    }
                    ConsumeClass(class) => {
                        for (start, end) in class.iter() {
                            if c < *start {
                                break;
                            } else if c > *end {
                                continue;
                            }
                            state.push_next(thread.inc_pc());
                            continue 'next_active;
                        }
                        thread.free(state);
                        break;
                    }
                    ConsumeOutlined(id) => {
                        // TODO: Avoid copy paste
                        let class = &self.bytecode.outlined_classes[*id];
                        for (start, end) in class.iter() {
                            if c < *start {
                                break;
                            } else if c > *end {
                                continue;
                            }
                            state.push_next(thread.inc_pc());
                            continue 'next_active;
                        }
                        thread.free(state);
                        break;
                    }
                    ConsumeAny => {
                        state.push_next(thread.inc_pc());
                        break;
                    }
                    Fork2(a, b) => {
                        let new_thread = thread.dup(state).with_pc(*b);
                        state.push_active(new_thread);
                        thread.pc = *a;
                    }
                    ForkN(branches) => {
                        let len = branches.len();
                        for pc in branches.iter().rev().take(len - 1) {
                            let new_thread = thread.dup(state).with_pc(*pc);
                            state.push_active(new_thread);
                        }
                        thread.pc = branches[0];
                    }
                    Jmp(target) => {
                        thread.pc = *target;
                    }
                    WriteReg(r) => {
                        thread.write_reg(*r as usize, state.input_pos, state);
                        thread.pc += 1;
                    }
                    Accept => {
                        state.accept(thread);
                        break;
                    }
                    Assertion(look) => match look {
                        Look::Start => {
                            if prev == Char::INPUT_BOUND {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        Look::End => {
                            if c == Char::INPUT_BOUND {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        Look::StartLF => {
                            if prev == Char::INPUT_BOUND || prev == '\n'.into() {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        Look::EndLF => {
                            if c == Char::INPUT_BOUND || c == '\n'.into() {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        Look::StartCRLF => {
                            if prev == Char::INPUT_BOUND
                                || prev == '\n'.into()
                                || (prev == '\r'.into() && c != '\n'.into())
                            {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        Look::EndCRLF => {
                            if c == Char::INPUT_BOUND
                                || c == '\r'.into()
                                || (c == '\n'.into() && prev != '\r'.into())
                            {
                                thread.pc += 1;
                            } else {
                                thread.free(state);
                                break;
                            }
                        }
                        _ => todo!(),
                    },
                    _ => {
                        thread.free(state);
                        break;
                    }
                }
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
        State::new(self.capture_count, self.bytecode.instructions.len(), 0)
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

        state.result_len = captures.len();

        let Input {
            subject,
            span: Span { from, to },
            anchored,
            first_match,
        } = input;

        let mut prev_char = find_prev_char(subject, from);

        state.input_pos = from;
        let first_thread = state.new_thread(0);
        first_thread.write_reg(0, from, state);
        state.push_active(first_thread);
        for c in subject[from..to].chars() {
            self.step(state, prev_char, c.into());
            prev_char = c.into();
            match &state.best_match {
                Some(_) if first_match || state.next.is_empty() => {
                    state.write_best_match(captures);
                    return true;
                }
                Some(_) => {
                    state.swap_and_advance_by(c.len_utf8());
                }
                None if !anchored => {
                    let thread = state.new_thread(0);
                    thread.write_reg(0, state.input_pos + c.len_utf8(), state);
                    state.push_next(thread);
                    state.swap_and_advance_by(c.len_utf8());
                }
                None => {
                    return false;
                }
            }
        }

        if to == subject.len() {
            self.step(state, prev_char, Char::INPUT_BOUND);
        } else {
            // TODO: Find a nicer way to do this
            let c = subject[to..subject.len()].chars().next().unwrap().into();
            self.step(state, prev_char, c);
        }

        if state.best_match.is_some() {
            state.write_best_match(captures);
            true
        } else {
            false
        }
    }
}
