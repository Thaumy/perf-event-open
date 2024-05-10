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

pub struct AsyncCowIter<'a> {
    inner: CowIter<'a>,
    waker: SyncSender<Waker>,
}

impl AsyncCowIter<'_> {
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
