use std::fs::File;
use std::future::Future;
use std::io::Result;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::task::AtomicWaker;

use crate::ffi::syscall;
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
    /// threads from outputting new data to the AUX area, and heavy operations
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
            use std::mem::MaybeUninit;
            use std::sync::atomic::{AtomicU8, Ordering};
            use std::thread;

            use crate::ffi::linux_syscall::{epoll_create1, epoll_ctl, epoll_wait, eventfd};

            let epoll = epoll_create1(libc::O_CLOEXEC)?;
            let mut event = libc::epoll_event {
                events: (libc::EPOLLIN | libc::EPOLLHUP) as _,
                u64: 0,
            };
            epoll_ctl(&epoll, libc::EPOLL_CTL_ADD, self.perf, &mut event)?;

            let close = eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC)?;
            let mut event = libc::epoll_event {
                events: libc::EPOLLIN as _,
                u64: 1,
            };
            epoll_ctl(&epoll, libc::EPOLL_CTL_ADD, &close, &mut event)?;

            let wait = Arc::new(Wait {
                close,
                state: AtomicU8::new(Wait::STATE_WAIT),
                waker: AtomicWaker::new(),
            });

            thread::spawn({
                let wait = Arc::clone(&wait);
                move || {
                    let mut events = [MaybeUninit::uninit()];

                    loop {
                        let Ok(event) = epoll_wait(&epoll, &mut events, -1).map(|it| &it[0]) else {
                            continue; // Error can only be `EINTR`, ignore it and try again.
                        };
                        if event.u64 == 1 {
                            break; // Async iter was dropped.
                        }
                        match event.events as _ {
                            libc::EPOLLIN => {
                                wait.state.store(Wait::STATE_WAKE, Ordering::Relaxed);
                                wait.waker.wake();
                            }
                            libc::EPOLLHUP => {
                                wait.state.store(Wait::STATE_HANG, Ordering::Relaxed);
                                wait.waker.wake();
                                break;
                            }
                            #[cfg(debug_assertions)]
                            _ => unreachable!(),
                            #[cfg(not(debug_assertions))]
                            _ => unsafe { std::hint::unreachable_unchecked() },
                        }
                    }
                }
            });

            Ok(AsyncCowIter { inner: self, wait })
        };
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        return {
            let _ = self.perf;
            Err(std::io::ErrorKind::Unsupported.into())
        };
    }
}

pub struct Wait {
    close: File,
    state: AtomicU8,
    waker: AtomicWaker,
}

impl Wait {
    const STATE_WAIT: u8 = 0;
    const STATE_WAKE: u8 = 1;
    const STATE_HANG: u8 = 2;
}

/// Asynchronous COW AUX area iterator.
pub struct AsyncCowIter<'a> {
    inner: CowIter<'a>,
    wait: Arc<Wait>,
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

        let wait = &this.wait;

        wait.waker.register(cx.waker());
        loop {
            break match wait.state.compare_exchange_weak(
                Wait::STATE_WAKE,
                Wait::STATE_WAIT,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Err(Wait::STATE_WAIT) => Poll::Pending,
                Ok(Wait::STATE_WAKE) => {
                    if let Some(cc) = this.inner.rb.lending_pop(max_chunk_len) {
                        Poll::Ready(Some(f(cc)))
                    } else {
                        Poll::Pending
                    }
                }
                Err(Wait::STATE_WAKE) => {
                    continue; // Spurious fail, try again.
                }
                Err(Wait::STATE_HANG) => {
                    Poll::Ready(this.inner.rb.lending_pop(max_chunk_len).map(f))
                }
                #[cfg(debug_assertions)]
                _ => unreachable!(),
                #[cfg(not(debug_assertions))]
                _ => unsafe { std::hint::unreachable_unchecked() },
            };
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

impl Drop for AsyncCowIter<'_> {
    fn drop(&mut self) {
        let _: Result<()> = syscall!(eventfd_write, &self.wait.close, 1);
    }
}
