use std::fs::File;
use std::future::Future;
use std::io::Result;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::mpsc::SyncSender;
use std::task::{Context, Poll, Waker};

use crate::sample::auxiliary::rb::Rb;
use crate::sample::rb::CowChunk;

/// COW (copy-on-write) AUX area iterator.
///
/// Same as [COW record iterator][crate::sample::iter::CowIter], but for AUX area.
pub struct CowIter<'a> {
    pub(in crate::sample::auxiliary) rb: Rb<'a>,
    pub(in crate::sample::auxiliary) perf: &'a File,
}

impl<'a> CowIter<'a> {
    /// Advances the iterator and returns the next value.
    ///
    /// `max_chunk_len` specifies the maximum length of a chunk
    /// that can be produced at one time, unlimited if `None`.
    ///
    /// If AUX area tracing is in happening, operations in the closure should
    /// be quick and cheap. Slow iteration of raw bytes may throttle kernel
    /// threads from outputting new data to the AUX area, and heavyd operations
    /// may affect the performance of the target process.
    pub fn next<F, R>(&mut self, f: F, max_chunk_len: Option<NonZeroUsize>) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>) -> R,
    {
        self.rb.lending_pop(max_chunk_len).map(f)
    }

    /// Creates an asynchronous iterator.
    pub fn into_async(self) -> Result<AsyncCowIter<'a>> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        return {
            use std::mem::{transmute, MaybeUninit};
            use std::sync::mpsc::sync_channel;
            use std::thread;

            use crate::ffi::linux_syscall::{epoll_create1, epoll_ctl, epoll_wait};

            let epoll = epoll_create1(libc::O_CLOEXEC)?;
            let mut event = libc::epoll_event {
                events: (libc::EPOLLIN | libc::EPOLLHUP) as _,
                u64: 0,
            };
            epoll_ctl(&epoll, libc::EPOLL_CTL_ADD, self.perf, &mut event)?;

            let (tx, rx) = sync_channel::<Waker>(1);

            thread::spawn(move || {
                let mut events = {
                    let src = [MaybeUninit::<libc::epoll_event>::uninit()];
                    // We don't care which event triggers epoll because we only monitor one event
                    // but `epoll_wait` requires a non-empty buffer
                    unsafe { transmute::<[_; 1], [_; 1]>(src) }
                };
                'exit: while let Ok(waker) = rx.recv() {
                    loop {
                        match epoll_wait(&epoll, &mut events, -1).map(|it| it[0].events as _) {
                            Ok(libc::EPOLLIN) => {
                                waker.wake();
                                break;
                            }
                            Ok(libc::EPOLLHUP) => {
                                drop(rx);
                                waker.wake();
                                break 'exit;
                            }
                            _ => (), // Error can only be `EINTR`, ignore it and try again.
                        }
                    }
                }
            });

            Ok(AsyncCowIter {
                inner: self,
                waker: tx,
            })
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        return {
            let _ = self.perf;
            Err(std::io::ErrorKind::Unsupported.into())
        };
    }
}

/// Asynchronous COW AUX area iterator.
pub struct AsyncCowIter<'a> {
    inner: CowIter<'a>,
    waker: SyncSender<Waker>,
}

impl AsyncCowIter<'_> {
    /// Attempt to pull out the next value, registering the current task for
    /// wakeup if the value is not yet available, and returning `None` if the
    /// iterator is exhausted.
    ///
    /// [`WakeUp::on_aux_bytes`][crate::config::WakeUp::on_aux_bytes]
    /// must be properly set to make this work.
    ///
    /// See also [`CowIter::next`].
    pub fn poll_next<F, R>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        f: F,
        max_chunk_len: Option<NonZeroUsize>,
    ) -> Poll<Option<R>>
    where
        F: FnOnce(CowChunk<'_>) -> R + Unpin,
    {
        let this = self.get_mut();

        if let Some(cc) = this.inner.rb.lending_pop(max_chunk_len) {
            return Poll::Ready(Some(f(cc)));
        }

        let waker = cx.waker().clone();
        match this.waker.send(waker) {
            Ok(()) => Poll::Pending,
            // The task we were monitoring exited, so the epoll thread died.
            // No more data needs to be produced.
            Err(_) => Poll::Ready(None),
        }
    }

    /// Advances the iterator and returns the next value.
    ///
    /// [`WakeUp::on_aux_bytes`][crate::config::WakeUp::on_aux_bytes]
    /// must be properly set to make this work.
    ///
    /// See also [`CowIter::next`].
    pub async fn next<F, R>(&mut self, f: F, max_chunk_len: Option<NonZeroUsize>) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>) -> R + Unpin,
    {
        struct Fut<I, F>(I, Option<F>, Option<NonZeroUsize>);

        impl<F, R> Future for Fut<&mut AsyncCowIter<'_>, F>
        where
            F: FnOnce(CowChunk<'_>) -> R + Unpin,
        {
            type Output = Option<R>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let Fut(iter, f, max_chunk_len) = self.get_mut();

                Pin::new(&mut **iter).poll_next(
                    cx,
                    |cc| {
                        let f = f.take();
                        // We only take `f` once, so there is always a value there.
                        let f = unsafe { f.unwrap_unchecked() };
                        f(cc)
                    },
                    *max_chunk_len,
                )
            }
        }

        Fut(self, Some(f), max_chunk_len).await
    }
}
