use std::fs::File;
use std::io::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arena::Arena;
use auxiliary::AuxTracer;
use iter::{CowIter, Iter};
use rb::Rb;
use record::{Parser, UnsafeParser};

use crate::count::Counter;
use crate::ffi::syscall::ioctl_arg;
use crate::ffi::{bindings as b, Metadata, PAGE_SIZE};

mod arena;
pub mod auxiliary;
pub mod iter;
pub mod rb;
pub mod record;

pub struct Sampler {
    perf: Arc<File>,
    arena: Arena,
    parser: Parser,
}

impl Sampler {
    pub(super) fn new(counter: &Counter, exp: u8) -> Result<Self> {
        let len = (1 + 2_usize.pow(exp as _)) * *PAGE_SIZE;
        let arena = Arena::new(&counter.perf, len, 0)?;

        // We only change the attr fields related to event config,
        // which are not used in `ChunkParser::from_attr`.
        let attr = unsafe { &*counter.attr.get() };
        let parser = Parser(UnsafeParser::from_attr(attr));

        Ok(Sampler {
            perf: Arc::clone(&counter.perf),
            arena,
            parser,
        })
    }

    pub fn iter(&self) -> Iter<'_> {
        let alloc = self.arena.as_slice();
        let metadata = unsafe { &mut *(alloc.as_ptr() as *mut Metadata) };
        let rb = Rb::new(
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L6212
            &alloc[*PAGE_SIZE..],
            unsafe { AtomicU64::from_ptr(&mut metadata.data_tail as _) },
            unsafe { AtomicU64::from_ptr(&mut metadata.data_head as _) },
        );
        Iter(CowIter {
            rb,
            perf: &self.perf,
            parser: &self.parser,
        })
    }

    pub fn parser(&self) -> &UnsafeParser {
        &self.parser.0
    }

    pub fn aux_tracer(&self, exp: u8) -> Result<AuxTracer<'_>> {
        let alloc = self.arena.as_slice();
        let metadata = unsafe { &mut *(alloc.as_ptr() as *mut Metadata) };
        AuxTracer::new(&self.perf, metadata, exp)
    }

    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    #[cfg(feature = "linux-4.7")]
    pub fn pause(&self) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_PAUSE_OUTPUT as _, 1)?;
        Ok(())
    }

    #[cfg(not(feature = "linux-4.7"))]
    pub fn pause(&self) -> Result<()> {
        crate::config::unsupported!()
    }

    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    #[cfg(feature = "linux-4.7")]
    pub fn resume(&self) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_PAUSE_OUTPUT as _, 0)?;
        Ok(())
    }

    #[cfg(not(feature = "linux-4.7"))]
    pub fn resume(&self) -> Result<()> {
        crate::config::unsupported!()
    }

    pub fn enable_counter_with(&self, max_samples: u32) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_REFRESH as _, max_samples as _)?;
        Ok(())
    }

    pub fn sample_on(&self, freq_or_count: u64) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_PERIOD as _, freq_or_count)?;
        Ok(())
    }

    fn metadata_inner(&self) -> *mut Metadata {
        let alloc_ptr = self.arena.as_slice().as_ptr();
        alloc_ptr as *mut Metadata
    }

    pub fn counter_time_enabled(&self) -> u64 {
        let metadata = self.metadata_inner();
        let metadata = unsafe { &mut *metadata };
        let time_enabled = unsafe { AtomicU64::from_ptr(&mut metadata.time_enabled as _) };
        time_enabled.load(Ordering::Relaxed)
    }

    pub fn counter_time_running(&self) -> u64 {
        let metadata = self.metadata_inner();
        let metadata = unsafe { &mut *metadata };
        let time_running = unsafe { AtomicU64::from_ptr(&mut metadata.time_running as _) };
        time_running.load(Ordering::Relaxed)
    }
}

// `Arena::ptr` is valid during the lifetime of `Sampler`.
unsafe impl Send for Sampler {}
