use crate::util::{Captures, Input, Match};

/// A Gregex
pub trait Regex {
    /// Returns true whenever the input matches the regex or not, without
    /// returning the bounds of the match. This returns true iff find returns
    /// Some(...), but it may be faster in some cases.
    fn is_match<'s>(&self, input: impl Into<Input<'s>>) -> bool {
        self.find(input.into().first_match(true)).is_some()
    }

    /// Match the regex against the input and returns the bounds of the match or
    /// None.
    fn find<'s>(&self, input: impl Into<Input<'s>>) -> Option<Match<'s>>;

    /// Returns an iterator over all non-overlapping match in the input.
    fn find_all<'s>(&self, input: impl Into<Input<'s>>) -> impl Iterator<Item = Match<'s>>;

    /// Match the regex against the input and returns a match with all its
    /// capture groups bounds or None If only the overall match is needed, you
    /// should prefer the use of `find` since it can be faster.
    fn find_captures<'s>(&self, input: impl Into<Input<'s>>) -> Option<Captures<'s>>;

    /// Rerturns an interator over all non-overlapping match in the input, with
    /// their capture group bounds. If only the overall match is needed, you
    /// should prefer the use of `find_all` since it can be faster.
    fn find_all_captures<'s>(
        &self,
        input: impl Into<Input<'s>>,
    ) -> impl Iterator<Item = Captures<'s>>;
}

/// Generic iterator for `find_all`.
/// It works by simply calling `find` in a loop.
///
/// You might wonder why this is not used as a default implementation for
/// `find_all` in [`Regex`], the reason is that it would either require dynamic
/// dispatch, or require adding Sized as a bound to Regex which would remove the object
/// safety of the trait.
pub struct FindAll<'r, 's, R> {
    regex: &'r R,
    input: Input<'s>,
}

impl<'r, 's, R> FindAll<'r, 's, R> {
    pub fn new(regex: &'r R, input: Input<'s>) -> Self {
        Self { regex, input }
    }
}

impl<'r, 's, R: Regex> Iterator for FindAll<'r, 's, R> {
    type Item = Match<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.input.span.valid() {
            return None;
        }
        let result = self.regex.find(self.input.clone())?;
        self.input.span.from = result.next_match_start();
        Some(result)
    }
}

// TODO: Find a way to avoid the copy paste

pub struct FindAllCaptures<'r, 's, R> {
    regex: &'r R,
    input: Input<'s>,
}

impl<'r, 's, R> FindAllCaptures<'r, 's, R> {
    pub fn new(regex: &'r R, input: Input<'s>) -> Self {
        Self { regex, input }
    }
}

impl<'r, 's, R: Regex> Iterator for FindAllCaptures<'r, 's, R> {
    type Item = Captures<'s>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.input.span.valid() {
            return None;
        }
        let result = self.regex.find_captures(self.input.clone())?;
        self.input.span.from = result.group0().next_match_start();
        Some(result)
    }
}
