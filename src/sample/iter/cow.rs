use std::fs::File;
use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::task::AtomicWaker;

use crate::ffi::syscall;
use crate::sample::rb::{CowChunk, Rb};
use crate::sample::record::Parser;

/// COW (copy-on-write) record iterator.
///
/// This type allows you to access the raw bytes of record in the
/// underlying ring-buffer directly without copy it to the outside.
pub struct CowIter<'a> {
    pub(in crate::sample) rb: Rb<'a>,
    pub(in crate::sample) perf: &'a File,
    pub(in crate::sample) parser: &'a Parser,
}

impl<'a> CowIter<'a> {
    /// Advances the iterator and returns the next value.
    ///
    /// If sampling is in happening, operations in the closure should be
    /// quick and cheap. Slow iteration of raw bytes may throttle kernel
    /// threads from outputting new data to the ring-buffer, and heavy
    /// operations may affect the performance of the target process.
    ///
    /// # Examples
    ///
    /// ``` rust
    /// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, Size};
    /// use perf_event_open::count::Counter;
    /// use perf_event_open::event::sw::Software;
    ///
    /// let event = Software::TaskClock;
    /// let target = (Proc::ALL, Cpu(0));
    ///
    /// let mut opts = Opts::default();
    /// opts.sample_on = SampleOn::Count(50_000); // 50us
    /// opts.sample_format.user_stack = Some(Size(8)); // Dump 8-bytes user stack in sample.
    ///
    /// let counter = Counter::new(event, target, &opts).unwrap();
    /// let sampler = counter.sampler(5).unwrap();
    /// let mut iter = sampler.iter().into_cow();
    ///
    /// counter.enable().unwrap();
    ///
    /// let mut skipped = 0;
    /// let it = loop {
    ///     let it = iter
    ///         .next(|cc, p| {
    ///             // ABI layout:
    ///             // u32 type
    ///             // u16 misc
    ///             // u16 size
    ///             // u64 len
    ///             // [u8; len] bytes
    ///
    ///             let ptr = cc.as_bytes().as_ptr();
    ///             let ty = ptr as *const u32;
    ///
    ///             // Only parse sample record with stack dumped.
    ///             if unsafe { *ty } == 9 {
    ///                 let len = unsafe { ptr.offset(8) } as *const u64;
    ///                 if unsafe { *len } > 0 {
    ///                     return Some(p.parse(cc));
    ///                 }
    ///             }
    ///
    ///             skipped += 1;
    ///             None
    ///         })
    ///         .flatten();
    ///
    ///     if let Some(it) = it {
    ///         break it;
    ///     }
    /// };
    ///
    /// println!("skipped: {}", skipped);
    /// println!("{:-?}", it);
    /// ```
    pub fn next<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>, &Parser) -> R,
    {
        self.rb.lending_pop().map(|cc| f(cc, self.parser))
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

/// Asynchronous COW record iterator.
pub struct AsyncCowIter<'a> {
    inner: CowIter<'a>,
    wait: Arc<Wait>,
}

impl AsyncCowIter<'_> {
    /// Attempt to pull out the next value, registering the current task for
    /// wakeup if the value is not yet available, and returning `None` if the
    /// iterator is exhausted.
    ///
    /// [`WakeUp::on`][crate::config::WakeUp::on] must be properly set to make this work.
    ///
    /// See also [`CowIter::next`].
    pub fn poll_next<F, R>(self: Pin<&mut Self>, cx: &mut Context<'_>, f: F) -> Poll<Option<R>>
    where
        F: FnOnce(CowChunk<'_>, &Parser) -> R + Unpin,
    {
        let this = self.get_mut();

        if let Some(cc) = this.inner.rb.lending_pop() {
            return Poll::Ready(Some(f(cc, this.inner.parser)));
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
                    if let Some(cc) = this.inner.rb.lending_pop() {
                        Poll::Ready(Some(f(cc, this.inner.parser)))
                    } else {
                        Poll::Pending
                    }
                }
                Err(Wait::STATE_WAKE) => {
                    continue; // Spurious fail, try again.
                }
                Err(Wait::STATE_HANG) => Poll::Ready(
                    this.inner
                        .rb
                        .lending_pop()
                        .map(|cc| f(cc, this.inner.parser)),
                ),
                #[cfg(debug_assertions)]
                _ => unreachable!(),
                #[cfg(not(debug_assertions))]
                _ => unsafe { std::hint::unreachable_unchecked() },
            };
        }
    }

    /// Advances the iterator and returns the next value.
    ///
    /// [`WakeUp::on`][crate::config::WakeUp::on] must be properly set to make this work.
    ///
    /// See also [`CowIter::next`].
    pub async fn next<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(CowChunk<'_>, &Parser) -> R + Unpin,
    {
        struct Fut<I, F>(I, Option<F>);

        impl<F, R> Future for Fut<&mut AsyncCowIter<'_>, F>
        where
            F: FnOnce(CowChunk<'_>, &Parser) -> R + Unpin,
        {
            type Output = Option<R>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let Fut(iter, f) = self.get_mut();

                Pin::new(&mut **iter).poll_next(cx, |cc, p| {
                    let f = f.take();
                    // We only take `f` once, so there is always a value there.
                    let f = unsafe { f.unwrap_unchecked() };
                    f(cc, p)
                })
            }
        }

        Fut(self, Some(f)).await
    }
}

impl Drop for AsyncCowIter<'_> {
    fn drop(&mut self) {
        let _: Result<()> = syscall!(eventfd_write, &self.wait.close, 1);
    }
}
