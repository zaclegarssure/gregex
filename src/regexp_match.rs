use std::ops::Index;

pub struct Match<'s> {
    pub subject: &'s str,
    indices: Box<[Option<usize>]>,
}

impl<'s> Match<'s> {
    pub fn new(subject: &'s str, indices: Box<[Option<usize>]>) -> Self {
        Self { subject, indices }
    }

    pub fn get(&self, group_index: usize) -> Option<&'s str> {
        let lower = self.indices.get(2 * group_index);
        let upper = self.indices.get(2 * group_index + 1);
        match (lower, upper) {
            (Some(Some(lower)), Some(Some(upper))) => Some(&self.subject[*lower..*upper]),
            _ => None,
        }
    }
}

impl Index<usize> for Match<'_> {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}
