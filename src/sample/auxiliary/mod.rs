use std::fs::File;
use std::io::Result;
use std::sync::atomic::AtomicU64;

use iter::{CowIter, Iter};
use rb::Rb;

use super::arena::Arena;
use crate::ffi::Metadata;

pub mod iter;
mod rb;

/// AUX tracer.
///
/// AUX tracer is used to export high bandwidth data streams to userspace,
/// such as instruction flow traces. Not all hardware supports this feature.
///
/// # Examples
///
/// ```rust
/// use std::fs::read_to_string;
/// use std::sync::mpsc::channel;
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::DynamicPmu;
///
/// let (tid_tx, tid_rx) = channel();
/// thread::spawn(move || {
///     tid_tx.send(unsafe { libc::gettid() }).unwrap();
///     loop {
///         std::hint::spin_loop();
///     }
/// });
///
/// // Intel PT
/// let ty = read_to_string("/sys/bus/event_source/devices/intel_pt/type");
/// # if ty.is_err() {
/// #     return;
/// # }
///
/// let event = DynamicPmu {
///     ty: ty.unwrap().lines().next().unwrap().parse().unwrap(),
///     config: 0,
///     config1: 0,
///     config2: 0,
///     config3: 0,
/// };
/// let target = (Proc(tid_rx.recv().unwrap() as _), Cpu::ALL);
/// let opts = Opts::default();
///
/// let counter  = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(10).unwrap();
/// let aux = sampler.aux_tracer(10).unwrap();
///
/// counter.enable().unwrap();
/// thread::sleep(Duration::from_millis(1));
/// counter.disable().unwrap();
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
/// }
/// while let Some(it) = aux.iter().next(None) {
///     let bytes = it.len();
///     println!("{:.2} KB", bytes as f64 / 1000.0);
/// }
/// ```
///
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
        Err(std::io::ErrorKind::Unsupported.into())
    }

    /// Get an iterator of the AUX area.
    pub fn iter(&self) -> Iter<'_> {
        Iter(CowIter {
            rb: Rb::new(self.arena.as_slice(), self.tail, self.head),
            perf: self.perf,
        })
    }
}
