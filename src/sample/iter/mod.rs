use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::record::{Priv, Record};

mod cow;

pub use cow::*;

/// Record iterator.
pub struct Iter<'a>(pub(super) CowIter<'a>);

impl<'a> Iter<'a> {
    /// Returns the underlying COW iterator.
    pub fn into_cow(self) -> CowIter<'a> {
        self.0
    }

    /// Creates an asynchronous iterator.
    pub fn into_async(self) -> Result<AsyncIter<'a>> {
        Ok(AsyncIter(self.0.into_async()?))
    }
}

impl Iterator for Iter<'_> {
    type Item = (Priv, Record);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next(|cc, p| p.parse(cc))
    }
}

/// Asynchronous record iterator.
pub struct AsyncIter<'a>(AsyncCowIter<'a>);

impl AsyncIter<'_> {
    /// Attempt to pull out the next value, registering the current task for
    /// wakeup if the value is not yet available, and returning `None` if the
    /// iterator is exhausted.
    ///
    /// [`WakeUpOn`][crate::config::WakeUpOn] must be properly set to make this work.
    pub fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<(Priv, Record)>> {
        let this = Pin::new(&mut self.get_mut().0);
        this.poll_next(cx, |cc, p| p.parse(cc))
    }

    /// Advances the iterator and returns the next value.
    ///
    /// [`WakeUpOn`][crate::config::WakeUpOn] must be properly set to make this work.
    pub async fn next(&mut self) -> Option<(Priv, Record)> {
        self.0.next(|cc, p| p.parse(cc)).await
    }
}
