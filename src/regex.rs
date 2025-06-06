//! Types and API for Regex matching
//!
//! This module defines the [`Regex`] struct, which is
//! a nice wrapper under one of the available [`RegexImpl`].

use std::error::Error;

use crate::thompson::pike_jit::JittedRegex;
use crate::thompson::pike_vm::PikeVM;
use crate::util::{Captures, Input, Match, Span};

type CompileError = Box<dyn Error + Send + Sync + 'static>;

/// A regular expression
pub struct Regex {
    engine: RegexEngine,
    // TODO: Replace that with group info once we have named
    // cg support
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
                pike_vm.exec(input.into().first_match(true), &mut state, &mut [])
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                jitted_regex.exec(input.into().first_match(true), &mut state, &mut [])
            }
        }
    }

    /// Match the regex against the input and returns the bounds of the match or
    /// None.
    pub fn find<'s>(&self, input: impl Into<Input<'s>>) -> Option<Match<'s>> {
        let input = input.into();
        let subject = input.subject;
        let mut result = [Span::invalid()];
        let found = match &self.engine {
            RegexEngine::PikeVM(pike_vm) => {
                let mut state = pike_vm.new_state();
                pike_vm.exec(input, &mut state, &mut result)
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                jitted_regex.exec(input, &mut state, &mut result)
            }
        };
        if !found {
            return None;
        }
        Some(Match::new(subject, result[0]))
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
            spans: [Span::invalid()],
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
                if !pike_vm.exec(input, &mut state, &mut spans) {
                    return None;
                }
            }
            RegexEngine::JittedRegex(jitted_regex) => {
                let mut state = jitted_regex.new_state();
                if !jitted_regex.exec(input, &mut state, &mut spans) {
                    return None;
                }
            }
        }
        Some(Captures::new(subject, spans.clone()))
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

    pub fn pike_vm(pattern: &str) -> Result<Self, CompileError> {
        Builder::new(pattern).pike_vm()
    }

    pub fn pike_jit(pattern: &str) -> Result<Self, CompileError> {
        Builder::new(pattern).pike_jit()
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub unicode: bool,
    pub case_insensitive: bool,
    pub cg: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            unicode: true,
            case_insensitive: false,
            cg: true,
        }
    }
}

impl From<Config> for regex_syntax::Parser {
    fn from(value: Config) -> Self {
        regex_syntax::ParserBuilder::new()
            .unicode(value.unicode)
            .case_insensitive(value.case_insensitive)
            .build()
    }
}

#[derive(Debug, Clone)]
pub struct Builder<'s> {
    pattern: &'s str,
    config: Config,
}

impl<'s> Builder<'s> {
    pub fn new(pattern: &'s str) -> Self {
        Self {
            pattern,
            config: Config::default(),
        }
    }

    pub fn unicode(mut self, value: bool) -> Self {
        self.config.unicode = value;
        self
    }

    pub fn case_insensitive(mut self, value: bool) -> Self {
        self.config.case_insensitive = value;
        self
    }

    pub fn cg(mut self, value: bool) -> Self {
        self.config.cg = value;
        self
    }

    pub fn pike_vm(self) -> Result<Regex, CompileError> {
        let pike_vm = PikeVM::new(self.pattern, self.config)?;
        let capture_count = pike_vm.capture_count();

        Ok(Regex {
            engine: RegexEngine::PikeVM(pike_vm),
            capture_count,
        })
    }

    pub fn pike_jit(self) -> Result<Regex, CompileError> {
        let pike_jit = JittedRegex::new(self.pattern, self.config)?;
        let capture_count = pike_jit.capture_count();
        Ok(Regex {
            engine: RegexEngine::JittedRegex(pike_jit),
            capture_count,
        })
    }
}

/// Iterator over all match in a regex.
pub struct AllMatch<'r, 's> {
    input: Input<'s>,
    spans: [Span; 1],
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
                // Add soft reset
                pike_vm.reset_state(state);
                pike_vm.exec(self.input.clone(), state, &mut self.spans)
            }
            EngineWithState::JittedRegex(jitted_regex, state) => {
                //jitted_regex.reset_state(state);
                jitted_regex.exec(self.input.clone(), state, &mut self.spans)
            }
        };
        if !result {
            return None;
        }
        let result = Match::new(self.input.subject, self.spans[0]);
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
                pike_vm.exec(self.input.clone(), state, &mut self.spans)
            }
            EngineWithState::JittedRegex(jitted_regex, state) => {
                // Actually we don't need to reset the state for, well, interesting reasons
                //jitted_regex.reset_state(state);
                jitted_regex.exec(self.input.clone(), state, &mut self.spans)
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
/// used in practice, since we use static dispatch, but it's there just to make
/// sure all engines maintain the same API, and in case we want to swtich to
/// dynamic dispatch at some point.
pub(crate) trait RegexImpl {
    /// State used by this engine. Every methods take a &mut State,
    /// in order to avoid repeated allocations when matching in a loop.
    type State;

    /// Return a new State for this engine
    fn new_state(&self) -> Self::State;

    /// Reset the state
    fn reset_state(&self, state: &mut Self::State);

    /// Finds the next match, if any, and fill the provided capture group array.
    /// If the given array is of size n, then only the n-first capture groups will be written.
    /// And if n is greater than the number of capture groups, then the remaining slots are not
    /// overwritten.
    /// This method is enough to write all higher-level functionalities of [`crate::Regex`].
    fn exec<'s>(&self, input: Input<'s>, state: &mut Self::State, captures: &mut [Span]) -> bool;
}
