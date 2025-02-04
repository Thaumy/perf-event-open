use std::fs::File;
use std::future::Future;
use std::io::Result;
use std::mem::{transmute, MaybeUninit};
use std::pin::Pin;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::task::{Context, Poll, Waker};
use std::thread;

use crate::ffi::syscall::{epoll_create1, epoll_ctl, epoll_wait};
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
    }
}

/// Asynchronous COW record iterator.
pub struct AsyncCowIter<'a> {
    inner: CowIter<'a>,
    waker: SyncSender<Waker>,
}

impl AsyncCowIter<'_> {
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

                if let Some(cc) = iter.inner.rb.lending_pop() {
                    let f = f.take();
                    // We only take `f` once, so there is always a value there.
                    let f = unsafe { f.unwrap_unchecked() };
                    return Poll::Ready(Some(f(cc, iter.inner.parser)));
                }

                let waker = cx.waker().clone();
                match iter.waker.send(waker) {
                    Ok(()) => Poll::Pending,
                    // The task we were monitoring exited, so the epoll thread died.
                    // No more data needs to be produced.
                    Err(_) => Poll::Ready(None),
                }
            }
        }

        Fut(self, Some(f)).await
    }
}
