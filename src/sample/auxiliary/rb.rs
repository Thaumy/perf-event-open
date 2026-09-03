use std::alloc::{alloc, handle_alloc_error, Layout};
use std::cell::UnsafeCell;
use std::num::NonZeroUsize;
use std::ptr::copy_nonoverlapping;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering as MemOrd};

use crate::sample::rb::CowChunk;

pub struct RingBuf<'a> {
    alloc: &'a [UnsafeCell<u8>],
    raw_tail: &'a AtomicU64,
    raw_head: &'a AtomicU64,
}

impl<'a> RingBuf<'a> {
    pub fn new(
        alloc: &'a [UnsafeCell<u8>],
        raw_tail: &'a AtomicU64,
        raw_head: &'a AtomicU64,
    ) -> Self {
        Self {
            alloc,
            raw_tail,
            raw_head,
        }
    }

    /// # Safety
    ///
    /// See [`crate::sample::rb::RingBuf::lending_pop`].
    pub unsafe fn lending_pop(&self, max_chunk_len: Option<NonZeroUsize>) -> Option<CowChunk<'a>> {
        let rb_ptr = self.alloc.as_ptr().cast::<u8>();
        let size = self.alloc.len();

        // Thread-safe because no other thread sets the tail.
        let raw_tail = unsafe { *self.raw_tail.as_ptr() };
        // About acquire:
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L720
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L99
        let raw_head = self.raw_head.load(MemOrd::Acquire);

        let data_len = raw_head.wrapping_sub(raw_tail) & (size as u64 - 1);
        if data_len == 0 {
            return None;
        }
        let chunk_len = match max_chunk_len {
            Some(max) => data_len.min(max.get() as _),
            None => data_len,
        };

        let tail = raw_tail & (size as u64 - 1);

        Some(match size as i64 - (tail + chunk_len) as i64 {
            d if d >= 0 => {
                let chunk = unsafe {
                    let ptr = rb_ptr.add(tail as _);
                    slice::from_raw_parts(ptr, chunk_len as _)
                };

                unsafe { CowChunk::borrowed(self.raw_tail, chunk) }
            }
            d => {
                let buf_layout = unsafe { Layout::from_size_align_unchecked(chunk_len as _, 1) };
                let buf_ptr = unsafe { alloc(buf_layout) };
                if buf_ptr.is_null() {
                    handle_alloc_error(buf_layout)
                }

                unsafe {
                    let hi_part_ptr = rb_ptr.add(tail as _);
                    let hi_part_len = (chunk_len + d as u64) as _;
                    copy_nonoverlapping(hi_part_ptr, buf_ptr, hi_part_len);

                    let lo_part_ptr = rb_ptr;
                    let lo_part_len = -d as _;
                    copy_nonoverlapping(lo_part_ptr, buf_ptr.add(hi_part_len), lo_part_len);
                }

                // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L723
                self.raw_tail.fetch_add(chunk_len as _, MemOrd::Release);

                unsafe { CowChunk::owned(buf_ptr, buf_layout) }
            }
        })
    }
}
