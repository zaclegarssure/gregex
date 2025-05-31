//! Types and API for Regex matching
//!
//! This module defines the [`Regex`] struct, which is
//! a nice wrapper under one of the available [`RegexImpl`].

use std::error::Error;

use crate::thompson::pike_jit::JittedRegex;
use crate::thompson::pike_vm::PikeVM;
use crate::util::{Captures, Input, Match, Span};

/// A regular expression
pub struct Regex {
    engine: RegexEngine,
    capture_count: usize,
}

impl Regex {
    /// Returns true whenever the input matches the regex or not, without
    /// returning the bounds of the match. This returns true iff find returns
    /// Some(...), but it may be faster in some cases.
    pub fn is_match<'s>(&self, input: impl Into<Input<'s>>) -> bool {
        match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let mut state = pike_vm.new_state();
                pike_vm.is_match(input, &mut state)
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                jitted_regex.is_match(input, &mut state)
            }
        }
    }

    /// Match the regex against the input and returns the bounds of the match or
    /// None.
    pub fn find<'s>(&self, input: impl Into<Input<'s>>) -> Option<Match<'s>> {
        match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let mut state = pike_vm.new_state();
                pike_vm.find(input.into(), &mut state)
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                jitted_regex.find(input.into(), &mut state)
            }
        }
    }

    /// Returns an iterator over all non-overlapping match in the input.
    pub fn find_all<'r, 's>(&'r self, input: impl Into<Input<'s>>) -> AllMatch<'r, 's> {
        let imp = match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let state = pike_vm.new_state();
                EngineWithState::PikeVM(pike_vm, state)
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let state = jitted_regex.new_state();
                EngineWithState::JittedRegex(jitted_regex, state)
            }
        };
        AllMatch {
            input: input.into(),
            imp,
        }
    }

    /// Match the regex against the input and returns a match with all its
    /// capture groups bounds or None If only the overall match is needed, you
    /// should prefer the use of `find` since it can be faster.
    pub fn find_captures<'s>(&self, input: impl Into<Input<'s>>) -> Option<Captures<'s>> {
        let input = input.into();
        let subject = input.subject;
        let mut spans = vec![Span::invalid(); self.capture_count].into_boxed_slice();
        match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let mut state = pike_vm.new_state();
                if !pike_vm.find_captures(input, &mut state, &mut spans) {
                    return None;
                }
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                if !jitted_regex.find_captures(input, &mut state, &mut spans) {
                    return None;
                }
            }
        }
        Some(Captures::new(subject, spans))
    }

    /// Rerturns an interator over all non-overlapping match in the input, with
    /// their capture group bounds. If only the overall match is needed, you
    /// should prefer the use of `find_all` since it can be faster.
    pub fn find_all_captures<'r, 's>(&'r self, input: impl Into<Input<'s>>) -> AllCaptures<'r, 's> {
        let imp = match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let state = pike_vm.new_state();
                EngineWithState::PikeVM(pike_vm, state)
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let state = jitted_regex.new_state();
                EngineWithState::JittedRegex(jitted_regex, state)
            }
        };
        let spans = vec![Span::invalid(); self.capture_count].into_boxed_slice();
        AllCaptures {
            input: input.into(),
            spans,
            imp,
        }
    }

    pub fn pike_vm(pattern: &str) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let pike_vm = PikeVM::new(pattern)?;
        let capture_count = pike_vm.capture_count();
        Ok(Self {
            engine: RegexEngine::PikeVM(pike_vm),
            capture_count,
        })
    }

    pub fn pike_jit(pattern: &str) -> Result<Self, Box<dyn Error + Send + Sync + 'static>> {
        let pike_jit = JittedRegex::new(pattern)?;
        let capture_count = pike_jit.capture_count();
        Ok(Self {
            engine: RegexEngine::JittedRegex(pike_jit),
            capture_count,
        })
    }
}

/// Iterator over all match in a regex.
pub struct AllMatch<'r, 's> {
    input: Input<'s>,
    imp: EngineWithState<'r>,
}

impl<'r, 's> Iterator for AllMatch<'r, 's> {
    type Item = Match<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.input.span.valid() {
            return None;
        }
        let result = match &mut self.imp {
            EngineWithState::PikeVM(pike_vm, state) => {
                pike_vm.reset_state(state);
                pike_vm.find(self.input.clone(), state)?
            }
            EngineWithState::JittedRegex(jitted_regex, state) => {
                jitted_regex.reset_state(state);
                jitted_regex.find(self.input.clone(), state)?
            }
        };
        self.input.span.from = result.next_match_start();
        Some(result)
    }
}

/// Iterator over all match and their capture groups.
pub struct AllCaptures<'r, 's> {
    input: Input<'s>,
    spans: Box<[Span]>,
    imp: EngineWithState<'r>,
}

impl<'r, 's> Iterator for AllCaptures<'r, 's> {
    type Item = Captures<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.input.span.valid() {
            return None;
        }
        let result = match &mut self.imp {
            EngineWithState::PikeVM(pike_vm, state) => {
                pike_vm.reset_state(state);
                pike_vm.find_captures(self.input.clone(), state, &mut self.spans)
            }
            EngineWithState::JittedRegex(jitted_regex, state) => {
                // Actually we don't need to reset the state for, well, interesting reasons
                jitted_regex.reset_state(state);
                jitted_regex.find_captures(self.input.clone(), state, &mut self.spans)
            }
        };
        if !result {
            return None;
        }
        // TODO: Don't clone the spans and instead reuse them
        let result = Captures::new(self.input.subject, self.spans.clone());
        self.input.span.from = result.group0().next_match_start();
        Some(result)
    }
}

/// A regex implementation. Used to dispatch to
/// the right version at runtime.
pub(crate) enum RegexEngine {
    PikeVM(PikeVM),
    JittedRegex(JittedRegex),
}

/// A regex implementation, with it's respective state.
/// Used when looking for all match.
pub(crate) enum EngineWithState<'r> {
    PikeVM(&'r PikeVM, <PikeVM as RegexImpl>::State),
    JittedRegex(&'r JittedRegex, <JittedRegex as RegexImpl>::State),
}

/// The Regex impl trait
///
/// Defines the lower-level api implemented by all regex engines in this crate.
/// For the user-facing one, see [`Regex`] just above. It turns out to not be
/// used in practisc, but it's there just to make sure all engines maintain the
/// same API, and in case we want to swtich to dynamic dispatch at some point.
pub(crate) trait RegexImpl {
    /// State used by this engine. Every methods take a &mut State,
    /// in order to avoid repeated allocations when matching in a loop.
    type State;

    fn new_state(&self) -> Self::State;

    fn reset_state(&self, state: &mut Self::State);

    /// Finds the next match, if any, in the given input.
    fn find<'s>(&self, input: Input<'s>, state: &mut Self::State) -> Option<Match<'s>>;

    /// Finds the next match, if any, and fill the provided capture group array.
    /// If the given array is of size n, then only the n-first capture groups will be written.
    /// And if n is greater than the number of capture groups, then the remaining slots are not
    /// overwritten.
    /// Technically only this method could be needed, where the array determine what we want.
    fn find_captures<'s>(
        &self,
        input: Input<'s>,
        state: &mut Self::State,
        captures: &mut [Span],
    ) -> bool;

    /// Returns true whenever the input matches the regex or not, without
    /// returning the bounds of the match. This returns true iff find returns
    /// Some(...), but it may be faster in some cases.
    fn is_match<'s>(&self, input: impl Into<Input<'s>>, state: &mut Self::State) -> bool {
        self.find(input.into().first_match(true), state).is_some()
    }
}
