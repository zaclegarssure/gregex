// use std::ops::Index;

// use crate::{JittedRegex, pike_jit::State};

// pub struct Match<'s> {
//     pub subject: &'s str,
//     start: usize,
//     end: usize,
//     captures: Option<Box<[Option<usize>]>>,
// }

// pub struct Matches<'s, 'r> {
//     pub subject: &'s str,
//     regex: &'r JittedRegex,
//     last_index: usize,
//     state: &'r mut State,
// }

// impl<'s, 'r> Matches<'s, 'r> {
//     // pub(crate) fn new(regex: &'r JittedRegex, subject: &'s str) -> Self {
//     //     Self {
//     //         subject,
//     //         regex,
//     //         last_index: 0,
//     //         state: &mut regex.new_state(),
//     //     }
//     // }

//     pub(crate) fn with_state(
//         regex: &'r JittedRegex,
//         subject: &'s str,
//         state: &'r mut State,
//     ) -> Self {
//         Self {
//             subject,
//             regex,
//             last_index: 0,
//             state,
//         }
//     }
// }

// impl<'s, 'r> Iterator for Matches<'s, 'r> {
//     type Item = Match<'s>;

//     fn next(&mut self) -> Option<Self::Item> {
//         match self.regex.exec_span(
//             self.subject,
//             self.last_index,
//             self.subject.len(),
//             false,
//             &mut self.state,
//         ) {
//             Some(next_match) => {
//                 if self.last_index == self.subject.len() {
//                     self.last_index += 1;
//                 }
//                 // Empty match, advance by one code point otherwise
//                 // it will return the same match again
//                 else if self.last_index == next_match.end() {
//                     // Okay this is crappy copy paste, I should find something better
//                     let upper_bound = Ord::min(self.last_index + 5, self.subject.len());
//                     self.last_index = self.subject.as_bytes()[self.last_index + 1..upper_bound]
//                         .iter()
//                         .position(|b| (*b as i8) >= -0x40)
//                         .map_or(upper_bound, |pos| pos + self.last_index + 1);
//                 } else {
//                     self.last_index = next_match.end();
//                 }
//                 Some(next_match)
//             }
//             None => {
//                 // Not strictly necessary but that way if someone spams next even after it returned None
//                 // then it wont be doing too much useless work.
//                 self.last_index = self.subject.len();
//                 None
//             }
//         }
//     }
// }

// impl<'s> Match<'s> {
//     pub fn new(
//         subject: &'s str,
//         start: usize,
//         end: usize,
//         captures: Option<Box<[Option<usize>]>>,
//     ) -> Self {
//         Self {
//             subject,
//             start,
//             end,
//             captures,
//         }
//     }

//     pub fn get(&self, group_index: usize) -> Option<&'s str> {
//         if group_index == 0 {
//             return Some(&self.subject[self.start..self.end]);
//         }
//         let lower = self.captures.as_ref().unwrap().get(2 * group_index);
//         let upper = self.captures.as_ref().unwrap().get(2 * group_index + 1);
//         match (lower, upper) {
//             (Some(Some(lower)), Some(Some(upper))) => Some(&self.subject[*lower..*upper]),
//             _ => None,
//         }
//     }

//     pub fn start(&self) -> usize {
//         self.start
//     }

//     pub fn end(&self) -> usize {
//         self.end
//     }

//     pub fn len(&self) -> usize {
//         self.end() - self.start()
//     }

//     #[must_use]
//     pub fn is_empty(&self) -> bool {
//         self.len() == 0
//     }

//     pub fn group_len(&self) -> usize {
//         match &self.captures {
//             Some(captures) => captures.len() / 2 + 1,
//             None => 1,
//         }
//     }
// }

// impl Index<usize> for Match<'_> {
//     type Output = str;

//     fn index(&self, index: usize) -> &Self::Output {
//         self.get(index).unwrap()
//     }
// }
