use std::borrow::Cow;
use std::cmp::Ordering as Ord;
use std::num::NonZeroUsize;
use std::ptr::copy_nonoverlapping;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering as MemOrd};

use crate::sample::rb::CowChunk;

pub struct Rb<'a> {
    alloc: &'a [u8],
    tail: &'a AtomicU64,
    head: &'a AtomicU64,
}

impl<'a> Rb<'a> {
    pub fn new(alloc: &'a [u8], tail: &'a AtomicU64, head: &'a AtomicU64) -> Self {
        Self { alloc, tail, head }
    }

    pub fn lending_pop(&self, max_chunk_len: Option<NonZeroUsize>) -> Option<CowChunk<'a>> {
        let rb_ptr = self.alloc.as_ptr();
        let size = self.alloc.len();

        // Thread safe since no more threads set the tail
        let tail = unsafe { *self.tail.as_ptr() };
        // About acquire:
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L720
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L99
        let head = self.head.load(MemOrd::Acquire) % size as u64;

        let chunk_len = {
            let len = match tail.cmp(&head) {
                Ord::Less => head - tail,
                Ord::Greater => size as u64 + head - tail,
                Ord::Equal => return None,
            };
            match max_chunk_len {
                Some(max) => len.min(max.get() as _),
                None => len,
            }
        };

        let new_tail = (tail + chunk_len) % size as u64;

        let chunk = match size as i64 - (tail + chunk_len) as i64 {
            d if d >= 0 => {
                let buf = unsafe {
                    let ptr = rb_ptr.add(tail as _);
                    slice::from_raw_parts(ptr, chunk_len as _)
                };
                Cow::Borrowed(buf)
            }
            d => {
                let mut buf = Vec::with_capacity(chunk_len as _);
                let buf_ptr = buf.as_mut_ptr();

                unsafe {
                    let hi_part_ptr = rb_ptr.add(tail as _);
                    let hi_part_len = (chunk_len + d as u64) as _;
                    copy_nonoverlapping(hi_part_ptr, buf_ptr, hi_part_len);

                    let lo_part_ptr = rb_ptr;
                    let lo_part_len = -d as _;
                    copy_nonoverlapping(lo_part_ptr, buf_ptr.add(hi_part_len), lo_part_len);
                    buf.set_len(chunk_len as _);
                }

                // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L723
                self.tail.store(new_tail, MemOrd::Release);

                Cow::Owned(buf)
            }
        };

        Some(CowChunk {
            tail: self.tail,
            new_tail,
            chunk,
        })
    }
}
