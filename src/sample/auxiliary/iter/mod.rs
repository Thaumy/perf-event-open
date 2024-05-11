use std::io::Result;
use std::num::NonZeroUsize;

mod cow;

pub use cow::*;

pub struct Iter<'a>(pub(super) CowIter<'a>);

impl<'a> Iter<'a> {
    pub fn next(&mut self, max_chunk_len: Option<NonZeroUsize>) -> Option<Vec<u8>> {
        self.0.next(|cc| cc.into_owned(), max_chunk_len)
    }

    pub fn into_cow(self) -> CowIter<'a> {
        self.0
    }

    pub fn into_async(self) -> Result<AsyncIter<'a>> {
        Ok(AsyncIter(self.0.into_async()?))
    }
}

pub struct AsyncIter<'a>(AsyncCowIter<'a>);

impl AsyncIter<'_> {
    pub async fn next(&mut self, max_chunk_len: Option<NonZeroUsize>) -> Option<Vec<u8>> {
        self.0.next(|cc| cc.into_owned(), max_chunk_len).await
    }
}
