//! Regex utils
//!
//! This module contains all utility types and functions used across the whole project,
//! and in particular across multiple engines.

use std::{
    cmp::{max, min},
    fmt,
    ops::Range,
};

/// Defines the input parameter to most matching methods on a [`crate::Regex`].
///
/// # Fields
/// - `subject`: The string to search in.
/// - `span`: The range within `subject` to search (default: the whole string).
/// - `anchored`: If true, only matches starting at the beginning of `span` are considered (default: false).
/// - `first_match`: If true, returns the first match found, not necessarily the leftmost (default: false).
///
/// Usually, you can just pass a `&str` to matching methods, but `Input` allows more control for advanced use cases.
#[derive(Clone)]
pub struct Input<'s> {
    pub subject: &'s str,
    pub span: Span,
    pub anchored: bool,
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

    /// Sets whether to return the first match found.
    pub fn first_match(mut self, value: bool) -> Self {
        self.first_match = value;
        self
    }

    /// Sets whenever to do an anchored match.
    pub fn anchored(mut self, value: bool) -> Self {
        self.anchored = value;
        self
    }

    pub fn span(mut self, value: Span) -> Self {
        self.span = value;
        self
    }

    pub fn from(mut self, value: usize) -> Self {
        self.span.from = value;
        self
    }

    pub fn to(mut self, value: usize) -> Self {
        self.span.to = value;
        self
    }

    /// Returns true if the span is valid and the boundaries are valid UTF-8 boundaries in the subject.
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

/// A span in a &str. Similar to [`std::ops::Range`], but
/// implements `Copy`. Plus, it uses `repr(C)` in order
/// to share it with the jitted code.
///
/// `from` is the start byte offset (inclusive), `to` is the end byte offset (exclusive).
#[derive(Copy, Debug, Clone)]
#[repr(C)]
pub struct Span {
    pub from: usize,
    pub to: usize,
}

impl Span {
    /// Returns true if the span is empty (from == to).
    pub fn empty(&self) -> bool {
        self.from == self.to
    }

    /// Returns true if the span is valid (from <= to).
    /// Invalid spans can be used to denote a no-match.
    pub fn valid(&self) -> bool {
        self.from <= self.to
    }

    /// Returns a span that is always considered invalid.
    pub fn invalid() -> Span {
        Span {
            from: usize::MAX,
            to: 0,
        }
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

/// Represents a successful non-capturing match. Contains only the bounds of the
/// overall match within the subject string. The span is guaranteed to be valid.
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

    /// Returns the matched substring.
    pub fn as_str(&self) -> &'s str {
        &self.subject[self.span.from..self.span.to]
    }

    /// Start in byte of this match
    pub fn start(&self) -> usize {
        self.span.from
    }

    /// End in byte of this match
    pub fn end(&self) -> usize {
        self.span.to
    }

    /// Returns the byte-index where the next non-overlapping match could start.
    /// This takes into account empty matches and advances at least one codepoint
    /// to avoid infinite loops.
    pub fn next_match_start(&self) -> usize {
        if self.span.empty() {
            if self.span.from == self.subject.len() {
                self.span.from + 1
            } else {
                // Must advance to next codepoint otherwise we would always return
                // the same empty match forever.
                // TODO: Find a less manual way to do this
                let b = self.subject.as_bytes()[self.span.from];
                if b < 0x80 {
                    self.span.from + 1
                } else if b < 0xE0 {
                    self.span.from + 2
                } else if b < 0xF0 {
                    self.span.from + 3
                } else {
                    self.span.from + 4
                }
            }
        } else {
            self.span.to
        }
    }
}

/// Represents a successful capturing match. Contains the bounds (if any) of all
/// capture groups defined in the pattern, including the implicit group 0 (the
/// overall match).
#[derive(Debug, Clone)]
pub struct Captures<'s> {
    subject: &'s str,
    spans: Box<[Span]>,
}

impl<'s> Captures<'s> {
    /// Returns the match for the given capture group index, or `None` if the
    /// group did not participate in the match.
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

    /// Returns the overall match (group 0).
    pub fn group0(&self) -> Match<'s> {
        // Must always be set
        self.get(0).unwrap()
    }

    pub fn new(subject: &'s str, spans: Box<[Span]>) -> Self {
        Self { subject, spans }
    }

    /// Returns the number of capture groups (including group 0).
    pub fn group_len(&self) -> usize {
        self.spans.len()
    }

    // TODO: Add an iterator over groups
    // and one over all matched groups maybe?
}

/// Represents a single Unicode code-point, or a special sentinel value used when
/// at the start or end of the input during the matching process.
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Char(u32);

impl Char {
    /// Sentinel value used to delimit the end and the beginning of an input string.
    pub const INPUT_BOUND: Char = Char(u32::MAX);

    /// Returns a range of `Char` matching all possible code points, including invalid ones.
    pub fn all() -> (Char, Char) {
        (Char(0), Self::INPUT_BOUND)
    }

    /// Returns a range of `Char` matching all valid Unicode code points.
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

/// This is a slight hack, since u8 is used to match bytes, but since Char is
/// essentially a wrapper around u32, encoding raw bytes with it is fine Note
/// however that fmt::Debug will make no sense if this is used to print a Char
/// which encodes a byte greater than ASCII::MAX
impl From<u8> for Char {
    fn from(value: u8) -> Self {
        (value as char).into()
    }
}

impl From<Char> for u32 {
    fn from(value: Char) -> Self {
        value.0
    }
}

/// Used to simplify the jit-code, since 32bit literals are encoded
/// using i32 rathern u32.
impl From<Char> for i32 {
    fn from(value: Char) -> Self {
        value.0.cast_signed()
    }
}

/// Given a (valid) position in a str, returns the previous character.
///
/// Since assertions require knowing which Char appeared before the current
/// one, and that at the beginning of the matching process we don't know
/// what is the first previous-char (except if from == 0) we must look for
/// it.
pub(crate) fn find_prev_char(s: &str, to: usize) -> Char {
    if to == 0 {
        return Char::INPUT_BOUND;
    }
    let mut from = to - 1;
    while !s.is_char_boundary(from) {
        from -= 1;
    }
    s[from..to].chars().next().unwrap().into()
}

/// A character interval where both bounds are inclusive. If the lower bound is
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
