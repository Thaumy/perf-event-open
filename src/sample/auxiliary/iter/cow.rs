use std::fs::File;
use std::num::NonZeroUsize;

use crate::sample::auxiliary::rb::Rb;
use crate::sample::rb::CowChunk;

pub struct CowIter<'a> {
    pub(in crate::sample::auxiliary) rb: Rb<'a>,
    pub(in crate::sample::auxiliary) perf: &'a File,
}

impl<'a> CowIter<'a> {
    pub fn next<F, R>(&mut self, f: F, max_chunk_len: Option<NonZeroUsize>) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>) -> R,
    {
        self.rb.lending_pop(max_chunk_len).map(f)
    }
}
