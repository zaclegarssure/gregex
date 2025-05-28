/*!
This modules contains all utils types and functions used accross the whole project,
and in particular accross multiple engines.
*/

use std::ops::Range;

/// Defines the input paramter to most matching methods on a [`crate::Regex`].
/// Since all values other than subject have a default value it's always
/// sufficient to only provide the subject string to all matching methods,
/// but for cases where we need more control (when finding all matches for instance)
/// this types come handy.
#[derive(Clone)]
pub struct Input<'s> {
    /// The subject string against which the regex is matched
    pub subject: &'s str,
    /// Perform the match within that span (but take the surroundings into accounts)
    /// Default: 0..subject.len()
    pub span: Span,
    /// Whenever the match should be anchored at the start of span.
    /// Default: false
    pub anchored: bool,
    /// Whenever the search should return the first match, or the left-most one.
    /// Default: false
    pub first_match: bool,
}

impl<'s> Input<'s> {
    pub fn new(subject: &'s str) -> Self {
        Self {
            subject,
            span: (0..subject.len()).into(),
            anchored: false,
            first_match: false,
        }
    }

    pub fn first_match(mut self, value: bool) -> Self {
        self.first_match = value;
        self
    }

    pub fn valid(&self) -> bool {
        self.span.valid()
            && self.subject.is_char_boundary(self.span.from)
            && self.subject.is_char_boundary(self.span.to)
    }
}

impl<'s> From<&'s str> for Input<'s> {
    fn from(subject: &'s str) -> Self {
        Self::new(subject)
    }
}

/// A span in a &str. Similar to [`std::range::Range`], but
/// implements Copy. Plus, it implements repr(C) in order
/// to share it with the jitted code.
#[derive(Copy, Debug, Clone)]
#[repr(C)]
pub struct Span {
    pub from: usize,
    pub to: usize,
}

impl Span {
    pub fn empty(&self) -> bool {
        self.from == self.to
    }

    pub fn valid(&self) -> bool {
        self.from <= self.to
    }

    pub fn invalid() -> Span {
        Span { from: 1, to: 0 }
    }
}

impl From<Range<usize>> for Span {
    fn from(value: Range<usize>) -> Self {
        Self {
            from: value.start,
            to: value.end,
        }
    }
}

impl From<Span> for Range<usize> {
    fn from(val: Span) -> Self {
        val.from..val.to
    }
}

/// Successful non-capturing match. Contains only the bounds of the
/// overall match.
#[derive(Copy, Debug, Clone)]
pub struct Match<'s> {
    pub subject: &'s str,
    pub span: Span,
}

impl<'s> Match<'s> {
    pub fn new(subject: &'s str, span: impl Into<Span>) -> Self {
        let span = span.into();
        Self { subject, span }
    }

    pub fn slice(&self) -> &str {
        &self.subject[self.span.from..self.span.to]
    }

    /// Returns the byte-index where the next non-overlapping
    /// match could start. This take into account empty match.
    pub fn next_match_start(&self) -> usize {
        if self.span.empty() && self.span.from < self.subject.len() {
            // Must advance to next codepoint otherwise we would always return
            // the same empty match forever.
            let range: Range<usize> = self.span.into();
            self.subject[range].len()
        } else {
            self.span.to
        }
    }
}

/// Successful capturing match. Contains the bounds (if any) of all capture groups
/// defined in the pattern. In particular this include the implicit capture-group
/// 0.
#[derive(Debug, Clone)]
pub struct Captures<'s> {
    subject: &'s str,
    spans: Box<[Span]>,
}

impl<'s> Captures<'s> {
    pub fn get(&self, group_index: usize) -> Option<Match<'s>> {
        let span = *self.spans.get(group_index)?;
        if !span.valid() {
            return None;
        }

        Some(Match {
            subject: self.subject,
            span,
        })
    }

    pub fn group0(&self) -> Match<'s> {
        // Must always be set
        self.get(0).unwrap()
    }

    pub fn new(subject: &'s str, spans: Box<[Span]>) -> Self {
        Self { subject, spans }
    }

    pub fn group_len(&self) -> usize {
        self.spans.len()
    }

    // TODO: Add an iterator over groups
    // and one over all matched groups maybe?
}
