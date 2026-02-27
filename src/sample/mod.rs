use std::fs::File;
use std::io::{Error, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use arena::Arena;
use auxiliary::AuxTracer;
use iter::{CowIter, Iter};
use rb::Rb;
use record::{Parser, UnsafeParser};

use crate::ffi::syscall::ioctl_arg;
use crate::ffi::{bindings as b, Attr, Metadata, PAGE_SIZE};

mod arena;
pub mod auxiliary;
pub mod iter;
pub mod rb;
pub mod record;

/// Event sampler.
///
/// This type provides the event sampling function of `perf_event_open`,
/// which can capture the context when the event happens, helpling us to
/// gain in-depth understanding of the system status at that time,
/// similar to the `perf record` command.
///
/// # Examples
///
/// ```rust
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc, Size};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::hw::Hardware;
/// # use perf_event_open::sample::record::Record;
///
/// // Count retired instructions on any process, CPU 0.
/// let event = Hardware::Instr;
/// let target = (Proc::ALL, Cpu(0));
///
/// let mut opts = Opts::default();
/// opts.sample_format.user_stack = Some(Size(32)); // Dump 32-bytes user stack.
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(10).unwrap(); // Allocate 2^10 pages to store samples.
///
/// counter.enable().unwrap();
/// thread::sleep(Duration::from_millis(10));
/// counter.disable().unwrap();
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     # if let (_, Record::Sample(s)) = it {
///     #     assert!(s.user_stack.is_some());
///     # }
/// }
/// ```
pub struct Sampler {
    perf: Arc<File>,
    arena: Arena,
    parser: Parser,
}

impl Sampler {
    pub(super) fn new(perf: Arc<File>, attr: &Attr, exp: u8) -> Result<Self> {
        let Some(len) = 2_usize
            .checked_pow(exp as u32)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(*PAGE_SIZE))
        else {
            return Err(Error::other("allocation size overflow"));
        };
        let arena = Arena::new(&perf, len, 0)?;

        Ok(Sampler {
            perf,
            arena,
            parser: Parser(UnsafeParser::from_attr(attr)),
        })
    }

    /// Returns a record iterator over the kernel ring-buffer.
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

    /// Record parser of the sampler.
    pub fn parser(&self) -> &UnsafeParser {
        &self.parser.0
    }

    /// Create an AUX tracer for this sampler.
    ///
    /// The AUX tracer needs a ring-buffer to store data,
    /// and 1 + 2^`exp` pages will be allocated for this.
    ///
    /// Multiple calls to this method just duplicates the existing AUX tracer,
    /// AUX tracers from the same sampler shares the same ring-buffer in the
    /// kernel space, so `exp` should be the same.
    pub fn aux_tracer(&self, exp: u8) -> Result<AuxTracer<'_>> {
        let alloc = self.arena.as_slice();
        let metadata = unsafe { &mut *(alloc.as_ptr() as *mut Metadata) };
        AuxTracer::new(&self.perf, metadata, exp)
    }

    /// Pause the ring-buffer output.
    ///
    /// A paused ring-buffer does not prevent generation of samples, but simply
    /// discards them. The discarded samples are considered lost, and cause a
    /// [`LostRecords`][record::lost::LostRecords] to be generated when possible.
    ///
    /// An overflow signal may still be triggered by the discarded sample even
    /// though the ring-buffer remains empty.
    ///
    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    pub fn pause(&self) -> Result<()> {
        #[cfg(feature = "linux-4.7")]
        return {
            ioctl_arg(&self.perf, b::PERF_IOC_OP_PAUSE_OUTPUT as _, 1)?;
            Ok(())
        };
        #[cfg(not(feature = "linux-4.7"))]
        return Err(std::io::ErrorKind::Unsupported.into());
    }

    /// Resume the ring-buffer output.
    ///
    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    pub fn resume(&self) -> Result<()> {
        #[cfg(feature = "linux-4.7")]
        return {
            ioctl_arg(&self.perf, b::PERF_IOC_OP_PAUSE_OUTPUT as _, 0)?;
            Ok(())
        };
        #[cfg(not(feature = "linux-4.7"))]
        return Err(std::io::ErrorKind::Unsupported.into());
    }

    /// Enables the counter until the maximum number of samples has been generated.
    ///
    /// The counter will be disabled if `max_samples` is reached.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::{thread, time::Duration};
    ///
    /// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn};
    /// use perf_event_open::count::Counter;
    /// use perf_event_open::event::sw::Software;
    ///
    /// let event = Software::TaskClock;
    /// let target = (Proc::ALL, Cpu(0));
    /// let mut opts = Opts::default();
    /// opts.sample_on = SampleOn::Count(1_000_000); // 1ms
    ///
    /// let counter = Counter::new(event, target, opts).unwrap();
    /// let sampler = counter.sampler(5).unwrap();
    ///
    /// sampler.enable_counter_with(10).unwrap();
    /// thread::sleep(Duration::from_millis(20));
    ///
    /// assert_eq!(sampler.iter().count(), 10);
    /// ```
    ///
    /// Furthermore, we can capture the overflow events by enabling I/O signaling from
    /// the perf event fd.
    ///
    /// On each overflow, `POLL_IN` is indicated if `max_samples` has not been reached.
    /// Otherwise, `POLL_HUP` is indicated.
    ///
    ///```rust
    /// # // Fork to avoid signal handler conflicts.
    /// # unsafe {
    /// #     let child = libc::fork();
    /// #     if child > 0 {
    /// #         let mut code = 0;
    /// #         libc::waitpid(child, &mut code as _, 0);
    /// #         assert_eq!(code, 0);
    /// #         return;
    /// #     }
    /// # }
    /// #
    /// # unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) };
    /// #
    /// # let result = std::panic::catch_unwind(|| {
    /// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn};
    /// use perf_event_open::count::Counter;
    /// use perf_event_open::event::sw::Software;
    /// use std::mem::MaybeUninit;
    /// use std::os::fd::AsRawFd;
    /// use std::ptr::null_mut;
    /// use std::sync::atomic::AtomicBool;
    /// use std::sync::atomic::Ordering as MemOrd;
    ///
    /// const MAX_SAMPLES: usize = 3;
    ///
    /// let event = Software::TaskClock;
    /// let target = (Proc::CURRENT, Cpu::ALL);
    /// let mut opts = Opts::default();
    /// opts.sample_on = SampleOn::Count(1_000_000); // 1ms
    ///
    /// let counter = Counter::new(event, target, opts).unwrap();
    ///
    /// // Enable I/O signals from perf event fd to the current process.
    /// let fd = counter.file().as_raw_fd();
    /// unsafe {
    ///     libc::fcntl(fd, libc::F_SETFL, libc::O_ASYNC);
    ///     // The value of `F_SETSIG` is 10, and libc crate does not have
    ///     // that binding (same as `POLL_IN` and `POLL_HUP` below).
    ///     libc::fcntl(fd, 10, libc::SIGIO);
    ///     libc::fcntl(fd, libc::F_SETOWN, libc::getpid());
    /// }
    ///
    /// static IN: AtomicBool = AtomicBool::new(false);
    /// static HUP: AtomicBool = AtomicBool::new(false);
    ///
    /// fn handler(num: i32, info: *const libc::siginfo_t) {
    ///     assert_eq!(num, libc::SIGIO);
    ///     match unsafe { *info }.si_code {
    ///         1 => IN.store(true, MemOrd::Relaxed),  // POLL_IN
    ///         6 => HUP.store(true, MemOrd::Relaxed), // POLL_HUP
    ///         _ => unreachable!(),
    ///     }
    /// }
    ///
    /// let act = libc::sigaction {
    ///     sa_sigaction: handler as _,
    ///     sa_mask: unsafe { MaybeUninit::zeroed().assume_init() },
    ///     sa_flags: libc::SA_SIGINFO,
    ///     sa_restorer: None,
    /// };
    /// unsafe { libc::sigaction(libc::SIGIO, &act as _, null_mut()) };
    ///
    /// let sampler = counter.sampler(5).unwrap();
    /// sampler.enable_counter_with(MAX_SAMPLES as _).unwrap();
    ///
    /// let iter = &mut sampler.iter();
    /// let mut count = 0;
    /// while !HUP.load(MemOrd::Relaxed) {
    ///     while IN.swap(false, MemOrd::Relaxed) {
    ///         count += iter.count();
    ///     }
    /// }
    /// count += iter.count();
    /// assert_eq!(count, MAX_SAMPLES);
    /// # });
    /// # if result.is_err() {
    /// #     unsafe { libc::abort() };
    /// # }
    /// #
    /// # unsafe { libc::exit(0) };
    /// ```
    pub fn enable_counter_with(&self, max_samples: u32) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_REFRESH as _, max_samples as _)?;
        Ok(())
    }

    /// Reset overflow condition.
    ///
    /// How to interpret `freq_or_count` depends on how the counter was created.
    /// This means that the new frequency will be applied if the counter was
    /// created with [`SampleOn::Freq`][crate::config::SampleOn], and so will the count.
    pub fn sample_on(&self, freq_or_count: u64) -> Result<()> {
        ioctl_arg(&self.perf, b::PERF_IOC_OP_PERIOD as _, freq_or_count)?;
        Ok(())
    }

    fn metadata_inner(&self) -> *mut Metadata {
        let alloc_ptr = self.arena.as_slice().as_ptr();
        alloc_ptr as *mut Metadata
    }

    /// Counter's enabled time.
    ///
    /// Same as [time][crate::count::Stat::time_enabled] returned by
    /// [`Counter::stat`][crate::count::Counter::stat], but much cheaper
    /// since the value is read from memory instead of system call.
    pub fn counter_time_enabled(&self) -> u64 {
        let metadata = self.metadata_inner();
        let metadata = unsafe { &mut *metadata };
        let time_enabled = unsafe { AtomicU64::from_ptr(&mut metadata.time_enabled as _) };
        time_enabled.load(Ordering::Relaxed)
    }

    /// Counter's running time.
    ///
    /// Same as [time][crate::count::Stat::time_running] returned by
    /// [`Counter::stat`][crate::count::Counter::stat], but much cheaper
    /// since the value is read from memory instead of system call.
    pub fn counter_time_running(&self) -> u64 {
        let metadata = self.metadata_inner();
        let metadata = unsafe { &mut *metadata };
        let time_running = unsafe { AtomicU64::from_ptr(&mut metadata.time_running as _) };
        time_running.load(Ordering::Relaxed)
    }
}

// `Arena::ptr` is valid during the lifetime of `Sampler`.
unsafe impl Send for Sampler {}
