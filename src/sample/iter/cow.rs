use std::fs::File;

use crate::sample::rb::{CowChunk, Rb};
use crate::sample::record::Parser;

pub struct CowIter<'a> {
    pub(in crate::sample) rb: Rb<'a>,
    pub(in crate::sample) perf: &'a File,
    pub(in crate::sample) parser: &'a Parser,
}

impl<'a> CowIter<'a> {
    pub fn next<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>, &Parser) -> R,
    {
        self.rb.lending_pop().map(|cc| f(cc, self.parser))
    }
}
