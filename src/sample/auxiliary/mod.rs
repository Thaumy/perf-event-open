use std::fs::File;
use std::io::Result;
use std::sync::atomic::AtomicU64;

use iter::{CowIter, Iter};
use rb::Rb;

use super::arena::Arena;
use crate::ffi::Metadata;

pub mod iter;
mod rb;

/// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/45bfb2e50471abbbfd83d40d28c986078b0d24ff>
pub struct AuxTracer<'a> {
    tail: &'a AtomicU64,
    head: &'a AtomicU64,
    arena: Arena,
    perf: &'a File,
}

impl<'a> AuxTracer<'a> {
    #[cfg(feature = "linux-4.1")]
    pub(crate) fn new(perf: &'a File, metadata: &'a mut Metadata, exp: u8) -> Result<Self> {
        metadata.aux_size = (2_usize.pow(exp as _) * *crate::ffi::PAGE_SIZE) as _;
        metadata.aux_offset = metadata.data_offset + metadata.data_size;

        let arena = Arena::new(perf, metadata.aux_size as _, metadata.aux_offset as _)?;
        let tail = unsafe { AtomicU64::from_ptr(&mut metadata.aux_tail as _) };
        let head = unsafe { AtomicU64::from_ptr(&mut metadata.aux_head as _) };

        Ok(Self {
            tail,
            head,
            arena,
            perf,
        })
    }

    #[cfg(not(feature = "linux-4.1"))]
    pub(crate) fn new(_: &File, _: &'a mut Metadata, _: u8) -> Result<Self> {
        crate::config::unsupported!()
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter(CowIter {
            rb: Rb::new(self.arena.as_slice(), self.tail, self.head),
            perf: self.perf,
        })
    }
}
