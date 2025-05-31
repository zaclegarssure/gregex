//! Regex utils
//!
//! This modules contains all utils types and functions used accross the whole project,
//! and in particular accross multiple engines.

use std::{
    cmp::{max, min},
    fmt,
    ops::Range,
};

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

    pub fn slice(&self) -> &'s str {
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

/// Represent a single unicode code-point, or a special sentinel value used when
/// we are at the start or the end of the input during the matching process.
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Char(u32);

impl Char {
    /// Used as a sentinel value to delimit the end and the begining
    /// of an input string.
    pub const INPUT_BOUND: Char = Char(u32::MAX);

    /// Return a range of Char matching all possible Char
    pub fn all() -> (Char, Char) {
        (Char(0), Self::INPUT_BOUND)
    }

    /// Return a range of Char matching all possible unicode codepoints.
    pub fn all_valid() -> (Char, Char) {
        (char::MIN.into(), char::MAX.into())
    }
}

impl fmt::Debug for Char {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::INPUT_BOUND {
            write!(f, "INPUT_BOUND")
        } else {
            write!(f, "{}", char::from_u32(self.0).unwrap())
        }
    }
}

impl From<char> for Char {
    fn from(value: char) -> Self {
        Self(value as u32)
    }
}

impl From<Char> for u32 {
    fn from(value: Char) -> Self {
        value.0
    }
}

/// A character interval where both bounds are inclusive. If the lowe bound is
/// greater than the upper bound, then the interval is considered empty.
#[derive(Debug, Clone, Copy)]
pub struct Interval(Char, Char);

impl Interval {
    /// Empty interval
    pub const EMPTY: Interval = Interval(Char(1), Char(0));
    /// Interval matching any (even invalid) code point
    pub const ALL: Interval = Interval(Char(0), Char(u32::MAX));
    /// Interval matching all valid code point
    pub const ALL_VALID: Interval = Interval(Char(char::MIN as u32), Char(char::MAX as u32));

    pub fn new(from: Char, to: Char) -> Self {
        Self(from, to)
    }

    pub fn is_empty(&self) -> bool {
        self.0 > self.1
    }

    /// Return the intersection between self and other.
    pub fn intersect(&self, other: &Interval) -> Interval {
        let start = max(self.0, other.0);
        let end = min(self.1, other.1);
        Interval(start, end)
    }

    /// Return the substraction between self and other. Since other can be
    /// strictly within self, this can result in two intervals.
    pub fn substract(&self, other: &Interval) -> (Interval, Interval) {
        // If no overlap, return self and empty
        if other.1 < self.0 || other.0 > self.1 {
            return (*self, Interval::EMPTY);
        }
        // Left part: from self.0 to one before other's start
        let left = if other.0 > self.0 {
            Interval(self.0, Char(u32::from(other.0) - 1))
        } else {
            Interval::EMPTY
        };
        // Right part: from one after other's end to self.1
        let right = if other.1 < self.1 {
            Interval(Char(u32::from(other.1) + 1), self.1)
        } else {
            Interval::EMPTY
        };
        (left, right)
    }
}

/// A sorted set of non-overlapping intervals, in increasing order.
#[derive(Debug, Clone)]
pub struct IntervalSet(Vec<Interval>);

impl IntervalSet {
    pub fn new(intervals: Vec<Interval>) -> Self {
        Self(intervals)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl IntervalSet {
    pub fn intersect_and_substract(
        mut self,
        mut other: IntervalSet,
    ) -> (IntervalSet, IntervalSet, IntervalSet) {
        let mut i = 0;
        let mut j = 0;
        let mut intersection = Vec::new();
        let mut self_only = Vec::new();
        let mut other_only = Vec::new();

        let self_intervals = &mut self.0;
        let other_intervals = &mut other.0;

        while i < self_intervals.len() && j < other_intervals.len() {
            let a = self_intervals[i];
            let b = other_intervals[j];

            let inter = a.intersect(&b);
            if !inter.is_empty() {
                intersection.push(inter);

                // Subtract intersection from a and b
                let (a_left, a_right) = a.substract(&b);
                if !a_left.is_empty() {
                    self_only.push(a_left);
                }
                if !a_right.is_empty() {
                    // Don't push yet, might overlap with next b
                    self_intervals[i] = a_right;
                    continue;
                }

                let (b_left, b_right) = b.substract(&a);
                if !b_left.is_empty() {
                    other_only.push(b_left);
                }
                if !b_right.is_empty() {
                    other_intervals[j] = b_right;
                    continue;
                }

                i += 1;
                j += 1;
            } else if a.1 < b.0 {
                self_only.push(a);
                i += 1;
            } else {
                other_only.push(b);
                j += 1;
            }
        }

        // Remaining intervals
        while i < self_intervals.len() {
            self_only.push(self_intervals[i]);
            i += 1;
        }
        while j < other_intervals.len() {
            other_only.push(other_intervals[j]);
            j += 1;
        }

        (
            IntervalSet(self_only),
            IntervalSet(intersection),
            IntervalSet(other_only),
        )
    }
}
