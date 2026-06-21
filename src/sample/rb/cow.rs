use std::alloc::{dealloc, Layout};
use std::borrow::Borrow;
use std::ptr::NonNull;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};

/// Copy-on-write chunk.
///
/// This type holds a reference to the underlying ring buffer data,
/// so it is necessary to drop this type as early as possible to
/// avoid the ring buffer being stuck due to insufficient space.
pub struct CowChunk<'a>(Inner<'a>);

impl CowChunk<'_> {
    pub(in crate::sample) unsafe fn borrowed<'a>(
        raw_tail: &'a AtomicU64,
        chunk: &'a [u8],
    ) -> CowChunk<'a> {
        CowChunk(Inner::Borrowed { raw_tail, chunk })
    }

    pub(in crate::sample) unsafe fn owned(ptr: *mut u8, layout: Layout) -> CowChunk<'static> {
        CowChunk(Inner::Owned(Chunk {
            ptr: NonNull::new_unchecked(ptr),
            layout,
        }))
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self.0 {
            Inner::Borrowed { chunk, .. } => chunk,
            Inner::Owned(Chunk { ptr, layout }) => unsafe {
                slice::from_raw_parts(ptr.as_ptr(), layout.size())
            },
        }
    }

    pub fn into_owned(self) -> Vec<u8> {
        // TODO:
        // For compatibility reasons, we will temporarily retain this
        // inefficient implementation until the next breaking release.
        self.as_bytes().to_vec()
    }
}

impl Borrow<[u8]> for CowChunk<'_> {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

enum Inner<'a> {
    Borrowed {
        raw_tail: &'a AtomicU64,
        chunk: &'a [u8],
    },
    Owned(Chunk),
}

impl Drop for Inner<'_> {
    fn drop(&mut self) {
        if let Inner::Borrowed { raw_tail, chunk } = self {
            // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L723
            raw_tail.fetch_add(chunk.len() as _, Ordering::Release);
        }
    }
}

struct Chunk {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl Drop for Chunk {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr.as_ptr(), self.layout) };
    }
}
