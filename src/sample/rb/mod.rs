use std::alloc::{alloc, handle_alloc_error, Layout};
use std::cell::UnsafeCell;
use std::cmp::Ordering as Ord;
use std::ptr::copy_nonoverlapping;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering as MemOrd};

pub use cow::CowChunk;

mod cow;

pub(super) struct RingBuf<'a> {
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
    /// There must be no live [`CowChunk`] previously returned by this method
    /// at the time of the call. The previous one must have been dropped.
    ///
    /// Violating this condition will cause the ring buffer to roll back its
    /// tail pointer, leading to UB (like UAF).
    pub unsafe fn lending_pop(&self) -> Option<CowChunk<'a>> {
        let rb_ptr = self.alloc.as_ptr().cast::<u8>();
        let size = self.alloc.len();

        // Thread-safe because no other thread sets the tail.
        let raw_tail = unsafe { *self.raw_tail.as_ptr() };
        // About acquire:
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L720
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L99
        let raw_head = self.raw_head.load(MemOrd::Acquire);

        if raw_tail == raw_head {
            return None;
        }

        let tail = raw_tail & (size as u64 - 1);

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L824
        // struct perf_event_header {
        //     u32 type; # 4 bytes
        //     u16 misc; # 2 bytes
        //     u16 size; # 2 bytes
        // };
        let chunk_len = {
            let d = size as u64 - tail;
            match d.cmp(&7) {
                Ord::Greater => unsafe {
                    let ptr = rb_ptr.add((tail + 6) as _);
                    *(ptr as *const u16)
                },
                Ord::Less => unsafe {
                    let ptr = rb_ptr.add((6 - d) as _);
                    *(ptr as *const u16)
                },
                Ord::Equal => unsafe {
                    let hi_part_ptr = rb_ptr.add((tail + 6) as _);
                    let lo_part_ptr = rb_ptr;
                    let buf = [*hi_part_ptr, *lo_part_ptr];
                    u16::from_ne_bytes(buf)
                },
            }
        };

        Some(match size as i64 - (tail + chunk_len as u64) as i64 {
            d if d >= 0 => {
                let chunk = unsafe {
                    let ptr = rb_ptr.add(tail as _);
                    slice::from_raw_parts(ptr, chunk_len as _)
                };

                unsafe { CowChunk::borrowed(self.raw_tail, chunk) }
            }
            d => {
                let buf_layout = unsafe { Layout::from_size_align_unchecked(chunk_len as _, 64) };
                let buf_ptr = unsafe { alloc(buf_layout) };
                if buf_ptr.is_null() {
                    handle_alloc_error(buf_layout)
                }

                unsafe {
                    let hi_part_ptr = rb_ptr.add(tail as _);
                    let hi_part_len = (chunk_len as i64 + d) as _;
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
