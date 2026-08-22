use std::fs::File;
use std::io::Result;
use std::slice;
use std::sync::atomic::AtomicU64;

use iter::{CowIter, Iter};
use rb::RingBuf;

use super::mmap::Mmap;
use crate::ffi::Metadata;

pub mod iter;
mod rb;

/// AUX tracer.
///
/// AUX tracer is used to export high-bandwidth data streams to user space, such as
/// instruction flow traces. Not all hardware supports this feature.
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
/// let counter = Counter::new(event, target, opts).unwrap();
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
    aux_tracer_alive: *mut bool,
    tail: &'a AtomicU64,
    head: &'a AtomicU64,
    mmap: Mmap,
    perf: &'a File,
}

impl<'a> AuxTracer<'a> {
    pub(crate) unsafe fn new(
        aux_tracer_alive: *mut bool,
        perf: &'a File,
        metadata: *mut Metadata,
        exp: u8,
    ) -> Result<Self> {
        #[cfg(feature = "linux-4.1")]
        return {
            use std::io::Error;

            use crate::ffi::PAGE_SIZE;

            let Some(len) = 2_usize
                .checked_pow(exp as u32)
                .and_then(|n| n.checked_mul(*PAGE_SIZE))
            else {
                return Err(Error::other("allocation size overflow"));
            };
            let aux_offset = unsafe { (*metadata).data_offset + (*metadata).data_size };
            unsafe { (*metadata).aux_size = len as _ };
            unsafe { (*metadata).aux_offset = aux_offset };

            let mmap = Mmap::new(perf, len, aux_offset as _)?;
            let tail = unsafe { AtomicU64::from_ptr(&mut (*metadata).aux_tail) };
            let head = unsafe { AtomicU64::from_ptr(&mut (*metadata).aux_head) };

            Ok(Self {
                aux_tracer_alive,
                tail,
                head,
                mmap,
                perf,
            })
        };
        #[cfg(not(feature = "linux-4.1"))]
        return {
            let _ = aux_tracer_alive;
            let _ = perf;
            let _ = metadata;
            let _ = exp;
            Err(std::io::ErrorKind::Unsupported.into())
        };
    }

    /// Get an iterator of the AUX area.
    pub fn iter(&self) -> Iter<'_> {
        let rb_ptr = self.mmap.as_ptr();
        let rb_len = self.mmap.len();
        let rb_alloc = unsafe { slice::from_raw_parts(rb_ptr, rb_len) };

        Iter(CowIter {
            rb: RingBuf::new(rb_alloc, self.tail, self.head),
            perf: self.perf,
        })
    }
}

impl Drop for AuxTracer<'_> {
    fn drop(&mut self) {
        unsafe { *self.aux_tracer_alive = false };
    }
}
