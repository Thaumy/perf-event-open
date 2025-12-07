use std::io::Result;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::task::{Context, Poll};

mod cow;

pub use cow::*;

/// AUX area iterator.
pub struct Iter<'a>(pub(super) CowIter<'a>);

impl<'a> Iter<'a> {
    /// Advances the iterator and returns the next value.
    ///
    /// `max_chunk_len` specifies the maximum length of a chunk
    /// that can be produced at one time, unlimited if `None`.
    pub fn next(&mut self, max_chunk_len: Option<NonZeroUsize>) -> Option<Vec<u8>> {
        self.0.next(|cc| cc.into_owned(), max_chunk_len)
    }

    /// Returns the underlying COW iterator.
    pub fn into_cow(self) -> CowIter<'a> {
        self.0
    }

    /// Creates an asynchronous iterator.
    pub fn into_async(self) -> Result<AsyncIter<'a>> {
        Ok(AsyncIter(self.0.into_async()?))
    }
}

/// Asynchronous AUX area iterator.
pub struct AsyncIter<'a>(AsyncCowIter<'a>);

impl AsyncIter<'_> {
    /// Attempt to pull out the next value, registering the current task for
    /// wakeup if the value is not yet available, and returning `None` if the
    /// iterator is exhausted.
    ///
    /// `max_chunk_len` specifies the maximum length of a chunk
    /// that can be produced at one time, unlimited if `None`.
    pub fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        max_chunk_len: Option<NonZeroUsize>,
    ) -> Poll<Option<Vec<u8>>> {
        let this = Pin::new(&mut self.get_mut().0);
        this.poll_next(cx, |cc| cc.into_owned(), max_chunk_len)
    }

    /// Advances the iterator and returns the next value.
    ///
    /// `max_chunk_len` specifies the maximum length of a chunk
    /// that can be produced at one time, unlimited if `None`.
    pub async fn next(&mut self, max_chunk_len: Option<NonZeroUsize>) -> Option<Vec<u8>> {
        self.0.next(|cc| cc.into_owned(), max_chunk_len).await
    }
}
