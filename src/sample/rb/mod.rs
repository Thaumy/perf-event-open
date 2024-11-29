use std::borrow::Cow;
use std::cmp::Ordering as Ord;
use std::mem::transmute;
use std::ptr::copy_nonoverlapping;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering as MemOrd};

pub use cow::CowChunk;

mod cow;

pub(super) struct Rb<'a> {
    alloc: &'a [u8],
    tail: &'a AtomicU64,
    head: &'a AtomicU64,
}

impl<'a> Rb<'a> {
    pub fn new(alloc: &'a [u8], tail: &'a AtomicU64, head: &'a AtomicU64) -> Self {
        Self { alloc, tail, head }
    }

    pub fn lending_pop(&self) -> Option<CowChunk<'a>> {
        let rb_ptr = self.alloc.as_ptr();
        let size = self.alloc.len();

        // Thread safe since no more threads set the tail
        let tail = unsafe { *self.tail.as_ptr() };
        // About acquire:
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L720
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L99
        let head = self.head.load(MemOrd::Acquire) % size as u64;

        if tail == head {
            return None;
        }

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
                    transmute::<[u8; 2], u16>(buf)
                },
            }
        };

        let new_tail = (tail + chunk_len as u64) % size as u64;

        let chunk = match size as i64 - (tail + chunk_len as u64) as i64 {
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
                    let hi_part_len = (chunk_len as i64 + d) as _;
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
