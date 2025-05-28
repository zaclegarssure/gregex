use crate::util::{Input, Match};

/// A Gregex
pub trait Regex {
    /// Returns true whenever the input matches the regex or not, without returning
    /// the bounds of the match. This returns true iff find returns Some(...), but
    /// it may be faster in some cases.
    fn is_match<'s>(&self, input: impl Into<Input<'s>>) -> bool {
        self.find(input.into().first_match(true)).is_some()
    }

    /// Match the regex agains the input and returns the bounds of the match or None.
    fn find<'s>(&self, input: impl Into<Input<'s>>) -> Option<Match<'s>>;

    /// Returns an iterator over all non-overlapping match in the input.
    fn find_all<'s>(&self, input: impl Into<Input<'s>>) -> impl Iterator<Item = Match<'s>>;
}

/// Generic iterator for `find_all`.
/// It works by simply calling `find` in a loop.
///
/// You might wonder why this is not used as a default implementation for
/// `find_all` in [`Regex`], the reason is that would either require dynamic
/// dispatch, or adding Sized as a bound to Regex which would remove the object
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
