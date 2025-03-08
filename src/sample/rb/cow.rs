use std::borrow::{Borrow, Cow};
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};

/// Copy-on-write chunk.
///
/// This type holds a reference to the underlying ring-buffer data,
/// so it is necessary to drop this type as early as possible to
/// avoid the ring buffer being stuck due to insufficient space.
pub struct CowChunk<'a> {
    pub(in crate::sample) tail: &'a AtomicU64,
    pub(in crate::sample) new_tail: u64,
    pub(in crate::sample) chunk: Cow<'a, [u8]>,
}

impl CowChunk<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.chunk
    }

    pub fn into_owned(mut self) -> Vec<u8> {
        match &mut self.chunk {
            Cow::Borrowed(b) => b.to_vec(),
            Cow::Owned(o) => {
                let mut vec = vec![];
                mem::swap(&mut vec, o);
                vec
            }
        }
    }
}

impl Borrow<[u8]> for CowChunk<'_> {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Drop for CowChunk<'_> {
    fn drop(&mut self) {
        if let Cow::Borrowed(_) = self.chunk {
            // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L723
            self.tail.store(self.new_tail, Ordering::Release);
        }
    }
}
