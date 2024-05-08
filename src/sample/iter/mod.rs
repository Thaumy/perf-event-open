use super::record::{Priv, Record};

mod cow;

pub use cow::*;

pub struct Iter<'a>(pub(super) CowIter<'a>);

impl<'a> Iter<'a> {
    pub fn into_cow(self) -> CowIter<'a> {
        self.0
    }
}

impl Iterator for Iter<'_> {
    type Item = (Priv, Record);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next(|cc, p| p.parse(cc))
    }
}
