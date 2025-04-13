use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::ffi::CStr;
use std::fs::File;
use std::io::{self, Error, ErrorKind, Result};
use std::mem::transmute;
use std::os::fd::AsRawFd;
use std::sync::Arc;

use super::sample::Sampler;
use crate::config::attr::from;
use crate::config::{Opts, Target};
use crate::event::Event;
use crate::ffi::syscall::{ioctl_arg, ioctl_argp, perf_event_open, read};
use crate::ffi::{bindings as b, Attr};

pub mod group;
mod stat;

pub use stat::*;

/// Event counter.
///
/// Linux has many performance events to help developers identify performance
/// issues with their programs. The [`perf_event_open`](https://man7.org/linux/man-pages/man2/perf_event_open.2.html)
/// system call exposes the performance event subsystem for us to monitor these events.
///
/// This type is the core of utilizing `perf_event_open`, which provides the
/// event counting functionality of `perf_event_open`, similar to the `perf stat` command.
///
/// # Permission
///
/// Access to performance monitoring and observability operations needs
/// `CAP_PERFMON` or `CAP_SYS_ADMIN` Linux capability, or consider adjusting
/// `/proc/sys/kernel/perf_event_paranoid` for users without these capabilities.
///
/// Possible values:
/// - -1: Allow use of (almost) all events by all users. Ignore mlock limit
///   after `perf_event_mlock_kb` without `CAP_IPC_LOCK`.
/// - \>= 0: Disallow raw and ftrace function tracepoint access.
/// - \>= 1: Disallow CPU event access.
/// - \>= 2: Disallow kernel profiling.
///
/// To make the adjusted `perf_event_paranoid` setting permanent, preserve it
/// in `/etc/sysctl.conf` (e.g., `kernel.perf_event_paranoid = <setting>`).
///
/// # Examples
///
/// ```rust
/// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, Size};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::hw::Hardware;
///
/// // Count retired instructions on current process, all CPUs.
/// let event = Hardware::Instr;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.sample_on = SampleOn::Freq(1000); // 1000 samples per second.
/// opts.sample_format.user_stack = Some(Size(8)); // Dump 8-bytes user stack in sample.
///
/// let counter = Counter::new(event, target, opts).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// fn fib(n: usize) -> usize {
///     match n {
///         0 => 0,
///         1 => 1,
///         n => fib(n - 1) + fib(n - 2),
///     }
/// }
/// std::hint::black_box(fib(30));
/// counter.disable().unwrap(); // Stop the counter.
///
/// let instrs = counter.stat().unwrap().count;
/// println!("{} instructions retired", instrs);
/// ```
pub struct Counter {
    pub(crate) target: Target,
    pub(crate) attr: UnsafeCell<Attr>,
    pub(crate) perf: Arc<File>,
    pub(crate) read_buf: UnsafeCell<Vec<u8>>,
}

impl Counter {
    /// Creates a new event counter.
    pub fn new(
        event: impl TryInto<Event, Error = io::Error>,
        target: impl Into<Target>,
        opts: impl Borrow<Opts>,
    ) -> Result<Self> {
        let target = target.into();
        let attr = from(event.try_into()?.0, opts.borrow())?;
        let flags = target.flags | b::PERF_FLAG_FD_CLOEXEC as u64;
        let perf = perf_event_open(&attr, target.pid, target.cpu, -1, flags)?;
        // Now there is only one event in the group, if in the future
        // this counter becomes the group leader, `CounterGroup::add`
        // will allocate a new buffer if `PERF_FORMAT_GROUP` is enabled.
        let read_buf = vec![0; Stat::read_buf_size(1, attr.read_format)];

        Ok(Self {
            target,
            attr: UnsafeCell::new(attr),
            perf: Arc::new(perf),
            read_buf: UnsafeCell::new(read_buf),
        })
    }

    /// Create a sampler for this counter.
    ///
    /// The sampler needs a ring-buffer to store metadata and records,
    /// and 1 + 2^`exp` pages will be allocated for this.
    ///
    /// A counter cannot have multiple samplers simultaneously.
    /// Attempting to create a new sampler while the previous one
    /// is still active will result in [`ErrorKind::AlreadyExists`].
    pub fn sampler(&self, exp: u8) -> Result<Sampler> {
        if Arc::strong_count(&self.perf) == 1 {
            // We only change the attr fields related to event config,
            // which are not used in `ChunkParser::from_attr`.
            let attr = unsafe { &*self.attr.get() };
            Sampler::new(Arc::clone(&self.perf), attr, exp)
        } else {
            // The kernel allows creating multiple samplers for a counter, these
            // samplers share the same ring buffer in kernel space and require
            // the same mmap length.
            //
            // Multiple samplers will result in an unsound `Send` impl, samplers
            // from different threads will race on the drop of COW chunks, which
            // may set the ring buffer head backwards.
            //
            // We prohibit users from creating multiple samplers per counter to
            // avoid the data race. Creating multiple samplers on the same counter
            // is usually useless, while the `Send` impl is much more useful.
            let error = "There is already an sampler attached to this counter.";
            Err(Error::new(ErrorKind::AlreadyExists, error))
        }
    }

    /// Returns the file handle opened by [`perf_event_open`](https://man7.org/linux/man-pages/man2/perf_event_open.2.html)
    /// system call for the current event.
    ///
    /// This might be useful if we want to interact with the handle directly.
    pub fn file(&self) -> &File {
        &self.perf
    }

    /// Returns the event ID.
    ///
    /// The event ID is a globally incremented ID used to distinguish the
    /// results of different counters.
    ///
    /// This is the same as [`Stat::id`], [`SiblingStat::id`] and [`RecordId::id`][crate::sample::record::RecordId::id].
    pub fn id(&self) -> Result<u64> {
        let mut id = 0;
        ioctl_argp(&self.perf, b::PERF_IOC_OP_ID as _, &mut id)?;
        Ok(id)
    }

    /// Enable counter.
    ///
    /// Counter will start to accumulate event counts.
    pub fn enable(&self) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_ENABLE as _, 0)?;
        Ok(())
    }

    /// Disable counter.
    ///
    /// Counter will stop to accumulate event counts.
    pub fn disable(&self) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_DISABLE as _, 0)?;
        Ok(())
    }

    /// Clear event count.
    ///
    /// This will only clear the event counts in the statistics,
    /// other fields (such as `time_enabled`) are not affected.
    pub fn clear_count(&self) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_RESET as _, 0)?;
        Ok(())
    }

    /// Returns counter statistics.
    pub fn stat(&self) -> Result<Stat> {
        // There could be only up to one reference to `read_buf` at the same time,
        // since `Counter` is not `Sync`.
        let buf = unsafe { &mut *self.read_buf.get() };

        read(&self.perf, buf)?;
        let buf = buf.as_mut_slice();
        let buf = unsafe { transmute::<&mut [_], &mut [u8]>(buf) };

        let ptr = buf.as_ptr();
        // We only change the attr fields related to event config,
        // there is nothing about `read_format`.
        let read_format = unsafe { &*self.attr.get() }.read_format;
        let stat = unsafe { Stat::from_ptr(ptr, read_format) };

        Ok(stat)
    }

    /// Attach a BPF program to an existing kprobe tracepoint event.
    ///
    /// The argument is a BPF program file that was created by a previous
    /// [`bpf`](https://man7.org/linux/man-pages/man2/bpf.2.html) system call.
    pub fn attach_bpf(&self, file: &File) -> Result<()> {
        ioctl_arg(
            &self.perf,
            b::PERF_IOC_OP_SET_BPF as _,
            file.as_raw_fd() as _,
        )?;
        Ok(())
    }

    /// Querying which BPF programs are attached to the
    /// existing kprobe tracepoint event.
    ///
    /// Returns the IDs of all BPF programs in all events attached to the tracepoint.
    ///
    /// If the buffer is not large enough to contain all IDs,
    /// it also indicates how many IDs were lost.
    ///
    /// Since `linux-4.16`: <https://github.com/torvalds/linux/commit/f371b304f12e31fe30207c41ca7754564e0ea4dc>
    #[cfg(feature = "linux-4.16")]
    pub fn query_bpf(&self, buf_len: u32) -> Result<(Vec<u32>, Option<u32>)> {
        // struct perf_event_query_bpf {
        //     u32 ids_len;
        //     u32 prog_cnt;
        //     u32 ids[0];
        // }

        use std::mem::MaybeUninit;
        let mut buf = vec![MaybeUninit::uninit(); (2 + buf_len) as _];
        buf[0] = MaybeUninit::new(buf_len); // set `ids_len`

        match ioctl_argp(
            &self.perf,
            b::PERF_IOC_OP_QUERY_BPF as _,
            buf.as_mut_slice(),
        ) {
            Ok(_) => {
                let prog_cnt = unsafe { buf[1].assume_init() };

                let ids = buf[2..2 + (prog_cnt as usize)].to_vec();
                let ids = unsafe { transmute::<Vec<_>, Vec<u32>>(ids) };

                Ok((ids, None))
            }
            Err(e) => {
                let option = e.raw_os_error();

                // `option` is always `Some` since `Error` is constructed
                // by `ioctl_argp` via `Error::last_os_error`.
                let errno = unsafe { option.unwrap_unchecked() };

                if errno == libc::ENOSPC {
                    let prog_cnt = unsafe { buf[1].assume_init() };

                    let ids = buf[2..].to_vec();
                    let ids = unsafe { transmute::<Vec<_>, Vec<u32>>(ids) };

                    return Ok((ids, Some(prog_cnt - buf_len)));
                }

                Err(e)
            }
        }
    }

    #[cfg(not(feature = "linux-4.16"))]
    pub fn query_bpf(&self, len: u32) -> Result<(Vec<u32>, Option<u32>)> {
        let _ = len;
        crate::config::unsupported!()
    }

    /// Add an ftrace filter to current event.
    pub fn with_ftrace_filter(&self, filter: &CStr) -> Result<()> {
        let ptr = filter.as_ptr() as *mut i8;

        // The following ioctl op just copies the bytes to kernel space,
        // so we don't have to worry about the mutable reference.
        let argp = unsafe { &mut *ptr };

        ioctl_argp(&self.perf, b::PERF_IOC_OP_SET_FILTER as _, argp)?;
        Ok(())
    }

    /// Switch to another event.
    ///
    /// This allows modifying an existing event without the overhead of
    /// closing and reopening a new counter.
    ///
    /// Currently this is supported only for breakpoint events.
    ///
    /// Since `linux-4.17`: <https://github.com/torvalds/linux/commit/32ff77e8cc9e66cc4fb38098f64fd54cc8f54573>
    #[cfg(feature = "linux-4.17")]
    pub fn switch_to<E>(&self, event: E) -> Result<()>
    where
        E: TryInto<Event, Error = io::Error>,
    {
        let Event(event_cfg): Event = event.try_into()?;

        // We can only access `self.attr` within the same thread,
        // so there is no potential data race.
        //
        // We will only change fields about event config, this will
        // not break any consumptions or states since these fields
        // are never used elsewhere after the counter is initialized.
        //
        // The following ioctl op just copies the modified attr to kernel space,
        // so we don't have to worry about the mutable reference.
        let attr = unsafe { &mut *self.attr.get() };
        attr.type_ = event_cfg.ty;
        attr.config = event_cfg.config;
        attr.__bindgen_anon_3.config1 = event_cfg.config1;
        attr.__bindgen_anon_4.config2 = event_cfg.config2;
        #[cfg(feature = "linux-6.3")]
        (attr.config3 = event_cfg.config3);
        attr.bp_type = event_cfg.bp_type;

        ioctl_argp(&self.perf, b::PERF_IOC_OP_MODIFY_ATTRS as _, attr)?;

        Ok(())
    }

    #[cfg(not(feature = "linux-4.17"))]
    pub fn switch_to<E>(&self, event: E) -> Result<()>
    where
        E: TryInto<Event, Error = io::Error>,
    {
        let _ = event;
        crate::config::unsupported!()
    }
}
