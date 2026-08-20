use std::fs::File;
use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::task::AtomicWaker;

use crate::ffi::syscall;
use crate::sample::rb::RingBuf;
use crate::sample::record::{Parser, RawRecord};

/// COW (copy-on-write) record iterator.
///
/// This type allows you to access the raw bytes of a record in the underlying
/// ring buffer directly without copying them out.
pub struct CowIter<'a> {
    pub(in crate::sample) rb: RingBuf<'a>,
    pub(in crate::sample) perf: &'a File,
    pub(in crate::sample) parser: &'a Parser,
    pub(in crate::sample) alive: *mut bool,
}

impl<'a> CowIter<'a> {
    /// Advances the iterator and returns the next value.
    ///
    /// If sampling is active, operations in the closure should be quick and cheap.
    /// Slow iteration of raw bytes may throttle kernel threads from outputting new data
    /// to the ring buffer, and heavy operations may affect the performance of
    /// the target process.
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
    /// opts.sample_format.user_stack = Some(Size(8)); // Dump an 8-byte user stack in each sample.
    ///
    /// let counter = Counter::new(event, target, &opts).unwrap();
    /// let sampler = counter.sampler(5).unwrap();
    /// let mut iter = sampler.iter().unwrap().into_cow();
    ///
    /// counter.enable().unwrap();
    ///
    /// let mut skipped = 0;
    /// let it = loop {
    ///     let Some(rr) = iter.lending_next() else {
    ///         skipped += 1;
    ///         continue;
    ///     };
    ///
    ///     // ABI layout:
    ///     // u32 type
    ///     // u16 misc
    ///     // u16 size
    ///     // u64 len
    ///     // [u8; len] bytes
    ///     let ptr = rr.as_raw().as_bytes().as_ptr();
    ///     let ty = ptr as *const u32;
    ///
    ///     // Only parse sample record with stack dumped.
    ///     if unsafe { *ty } == 9 {
    ///         let len = unsafe { ptr.offset(8) } as *const u64;
    ///         if unsafe { *len } > 0 {
    ///             break rr.parse();
    ///         }
    ///     }
    /// };
    ///
    /// println!("skipped: {}", skipped);
    /// println!("{:-?}", it);
    /// ```
    pub fn lending_next(&mut self) -> Option<RawRecord<'_>> {
        let chunk = unsafe { self.rb.lending_pop() }?;
        Some(RawRecord {
            chunk,
            parser: self.parser,
        })
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
                events: libc::EPOLLIN as _,
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
                        if event.events & (libc::EPOLLHUP | libc::EPOLLERR) as u32 > 0 {
                            wait.state.store(Wait::STATE_HANG, Ordering::Relaxed);
                            wait.waker.wake();
                            break;
                        }
                        if event.events & libc::EPOLLIN as u32 > 0 {
                            wait.state.store(Wait::STATE_WAKE, Ordering::Relaxed);
                            wait.waker.wake();
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

impl Drop for CowIter<'_> {
    fn drop(&mut self) {
        unsafe { *self.alive = false };
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

impl<'a> AsyncCowIter<'a> {
    unsafe fn poll(&mut self, cx: &Context<'_>) -> Poll<Option<RawRecord<'a>>> {
        if let Some(chunk) = unsafe { self.inner.rb.lending_pop() } {
            return Poll::Ready(Some(RawRecord {
                parser: self.inner.parser,
                chunk,
            }));
        }

        let wait = &self.wait;

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
                    if let Some(chunk) = unsafe { self.inner.rb.lending_pop() } {
                        Poll::Ready(Some(RawRecord {
                            parser: self.inner.parser,
                            chunk,
                        }))
                    } else {
                        Poll::Pending
                    }
                }
                Err(Wait::STATE_WAKE) => {
                    continue; // Spurious fail, try again.
                }
                Err(Wait::STATE_HANG) => Poll::Ready(unsafe { self.inner.rb.lending_pop() }.map(
                    |chunk| RawRecord {
                        parser: self.inner.parser,
                        chunk,
                    },
                )),
                #[cfg(debug_assertions)]
                _ => unreachable!(),
                #[cfg(not(debug_assertions))]
                _ => unsafe { std::hint::unreachable_unchecked() },
            };
        }
    }

    /// Attempt to pull out the next value, registering the current task for
    /// wakeup if the value is not yet available, and returning `None` if the
    /// iterator is exhausted.
    ///
    /// [`WakeUp::on`][crate::config::WakeUp::on] must be properly set to make this work.
    ///
    /// See also [`CowIter::lending_next`].
    pub fn poll_lending_next(&mut self, cx: &Context<'_>) -> Poll<Option<RawRecord<'_>>> {
        unsafe { self.poll(cx) }
    }

    /// Advances the iterator and returns the next value.
    ///
    /// [`WakeUp::on`][crate::config::WakeUp::on] must be properly set to make this work.
    ///
    /// See also [`CowIter::lending_next`].
    pub async fn lending_next(&mut self) -> Option<RawRecord<'_>> {
        struct Fut<I>(I);

        impl<'b> Future for Fut<&'b mut AsyncCowIter<'_>> {
            type Output = Option<RawRecord<'b>>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let Fut(iter) = self.get_mut();
                unsafe { iter.poll(cx) }
            }
        }

        Fut(self).await
    }
}

impl Drop for AsyncCowIter<'_> {
    fn drop(&mut self) {
        let _: Result<()> = syscall!(eventfd_write, &self.wait.close, 1);
    }
}
