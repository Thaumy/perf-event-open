use std::cell::UnsafeCell;
use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::ptr::addr_of_mut;
use std::sync::atomic::{compiler_fence, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::{hint, ptr, slice};

use auxiliary::AuxTracer;
use iter::{CowIter, Iter};
use mmap::Mmap;
use rb::RingBuf;
use record::{Parser, UnsafeParser};

use crate::ffi::{bindings as b, syscall, Attr, Metadata, PAGE_SIZE};

pub mod auxiliary;
pub mod iter;
mod mmap;
pub mod rb;
pub mod record;

/// Event sampler.
///
/// This type provides the event sampling function of `perf_event_open`,
/// which can capture the context when the event happens, helping us to
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
/// opts.sample_format.user_stack = Some(Size(32)); // Dump a 32-byte user stack.
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(10).unwrap(); // Use 2^10 pages for the sample ring buffer.
///
/// counter.enable().unwrap();
/// thread::sleep(Duration::from_millis(10));
/// counter.disable().unwrap();
///
/// for it in sampler.iter().unwrap() {
///     println!("{:-?}", it);
///     # if let (_, Record::Sample(s)) = it {
///     #     assert!(s.user_stack.is_some());
///     # }
/// }
/// ```
pub struct Sampler {
    perf: Arc<File>,
    mmap: Mmap,
    parser: Parser,
    iter_alive: UnsafeCell<bool>,
    aux_tracer_alive: UnsafeCell<bool>,
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
        let mmap = Mmap::new(&perf, len, 0)?;

        Ok(Sampler {
            perf,
            mmap,
            parser: Parser(UnsafeParser::from_attr(attr)),
            iter_alive: UnsafeCell::new(false),
            aux_tracer_alive: UnsafeCell::new(false),
        })
    }

    /// Returns a record iterator over the kernel ring buffer.
    ///
    /// There could be only up to one iterator over the sampler simultaneously,
    /// or this will return `None`.
    pub fn iter(&self) -> Option<Iter<'_>> {
        // `Self` and `CowIter` are guaranteed to run on the same thread,
        // so there is no data race.
        let iter_alive = self.iter_alive.get();
        if unsafe { ptr::replace(iter_alive, true) } {
            return None;
        }

        let rb = {
            let mmap_ptr = self.mmap.as_ptr();

            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L6212
            let page_size = *PAGE_SIZE;
            let rb_ptr = unsafe { mmap_ptr.add(page_size) }.cast::<UnsafeCell<u8>>();
            let rb_len = self.mmap.len() - page_size;
            let rb_alloc = unsafe { slice::from_raw_parts(rb_ptr, rb_len) };

            let metadata = mmap_ptr as *mut Metadata;
            let rb_tail = unsafe { AtomicU64::from_ptr(addr_of_mut!((*metadata).data_tail)) };
            let rb_head = unsafe { AtomicU64::from_ptr(addr_of_mut!((*metadata).data_head)) };

            RingBuf::new(rb_alloc, rb_tail, rb_head)
        };

        Some(Iter(CowIter {
            rb,
            perf: &self.perf,
            parser: &self.parser,
            alive: iter_alive,
        }))
    }

    /// Record parser of the sampler.
    pub fn parser(&self) -> &UnsafeParser {
        &self.parser.0
    }

    /// Create an AUX tracer for this sampler.
    ///
    /// The AUX tracer needs a ring buffer to store data, and 2^`exp` pages will
    /// be allocated for this.
    ///
    /// A sampler cannot have multiple AUX tracers simultaneously.
    /// Attempting to create a new AUX tracer while the previous one is still active
    /// will result in [`ErrorKind::AlreadyExists`].
    pub fn aux_tracer(&self, exp: u8) -> Result<AuxTracer<'_>> {
        // `Self` and `AuxTracer` are guaranteed to run on the same thread,
        // so there is no data race.
        let aux_tracer_alive = self.aux_tracer_alive.get();
        if unsafe { ptr::replace(aux_tracer_alive, true) } {
            let error = "There is already an AUX tracer attached to this sampler.";
            return Err(Error::new(ErrorKind::AlreadyExists, error));
        }

        let metadata = self.mmap.as_ptr() as *mut Metadata;
        match unsafe { AuxTracer::new(aux_tracer_alive, &self.perf, metadata, exp) } {
            Ok(o) => Ok(o),
            Err(e) => {
                unsafe { *aux_tracer_alive = false };
                Err(e)
            }
        }
    }

    /// Pause the ring buffer output.
    ///
    /// A paused ring buffer does not prevent generation of samples, but simply
    /// discards them. The discarded samples are considered lost, and cause a
    /// [`LostRecords`][record::lost::LostRecords] to be generated when possible.
    ///
    /// An overflow signal may still be triggered by the discarded sample even
    /// though the ring buffer remains empty.
    ///
    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    pub fn pause(&self) -> Result<()> {
        #[cfg(feature = "linux-4.7")]
        return {
            syscall!(
                unsafe,
                ioctl_arg,
                &self.perf,
                b::PERF_IOC_OP_PAUSE_OUTPUT as u64,
                1
            )?;
            Ok(())
        };
        #[cfg(not(feature = "linux-4.7"))]
        return Err(std::io::ErrorKind::Unsupported.into());
    }

    /// Resume the ring buffer output.
    ///
    /// Since `linux-4.7`: <https://github.com/torvalds/linux/commit/86e7972f690c1017fd086cdfe53d8524e68c661c>
    pub fn resume(&self) -> Result<()> {
        #[cfg(feature = "linux-4.7")]
        return {
            syscall!(
                unsafe,
                ioctl_arg,
                &self.perf,
                b::PERF_IOC_OP_PAUSE_OUTPUT as u64,
                0
            )?;
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
    /// assert_eq!(sampler.iter().unwrap().count(), 10);
    /// ```
    ///
    /// Furthermore, we can capture the overflow events by enabling I/O signaling
    /// from the perf event fd.
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
    /// use std::os::fd::AsRawFd;
    /// use std::ptr::null_mut;
    /// use std::sync::atomic::AtomicBool;
    /// use std::sync::atomic::Ordering;
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
    ///         1 => IN.store(true, Ordering::Relaxed),  // POLL_IN
    ///         6 => HUP.store(true, Ordering::Relaxed), // POLL_HUP
    ///         _ => unreachable!(),
    ///     }
    /// }
    ///
    /// let act = libc::sigaction {
    ///     sa_sigaction: handler as _,
    ///     sa_mask: unsafe { std::mem::zeroed() },
    ///     sa_flags: libc::SA_SIGINFO,
    ///     sa_restorer: None,
    /// };
    /// unsafe { libc::sigaction(libc::SIGIO, &act as _, null_mut()) };
    ///
    /// let sampler = counter.sampler(5).unwrap();
    /// sampler.enable_counter_with(MAX_SAMPLES as _).unwrap();
    ///
    /// let iter = &mut sampler.iter().unwrap();
    /// let mut count = 0;
    /// while !HUP.load(Ordering::Relaxed) {
    ///     while IN.swap(false, Ordering::Relaxed) {
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
        syscall!(
            unsafe,
            ioctl_arg,
            &self.perf,
            b::PERF_IOC_OP_REFRESH as u64,
            max_samples as u64
        )?;
        Ok(())
    }

    /// Reset overflow condition.
    ///
    /// How to interpret `freq_or_count` depends on how the counter was created.
    /// This means that the new frequency will be applied if the counter was
    /// created with [`SampleOn::Freq`][crate::config::SampleOn], and so will the count.
    pub fn sample_on(&self, freq_or_count: u64) -> Result<()> {
        // The following ioctl op simply copies the value to kernel space, so it
        // does not violate immutability.
        let addr = ptr::from_ref(&freq_or_count) as u64;
        syscall!(
            unsafe,
            ioctl_arg,
            &self.perf,
            b::PERF_IOC_OP_PERIOD as u64,
            addr,
        )?;
        Ok(())
    }

    /// Returns the latest `(time_enabled, time_running)` snapshot.
    ///
    /// This is cheaper than [`Counter::stat`][crate::count::Counter::stat], but the
    /// values may be stale while the counter is active.
    ///
    /// This is only reliable when the event monitors the calling thread. Otherwise,
    /// the values may be inconsistent on weakly ordered architectures.
    pub fn counter_time(&self) -> (u64, u64) {
        let metadata = self.mmap.as_ptr() as *mut Metadata;

        let lock = unsafe { AtomicU32::from_ptr(addr_of_mut!((*metadata).lock)) };
        let time_enabled = unsafe { AtomicU64::from_ptr(addr_of_mut!((*metadata).time_enabled)) };
        let time_running = unsafe { AtomicU64::from_ptr(addr_of_mut!((*metadata).time_running)) };

        loop {
            let seq = lock.load(Ordering::Relaxed);
            if seq & 1 == 1 {
                hint::spin_loop();
                continue;
            }
            compiler_fence(Ordering::SeqCst);

            let time_enabled = time_enabled.load(Ordering::Relaxed);
            let time_running = time_running.load(Ordering::Relaxed);

            compiler_fence(Ordering::SeqCst);
            if lock.load(Ordering::Relaxed) == seq {
                return (time_enabled, time_running);
            }
            hint::spin_loop();
        }
    }
}

// `Mmap::ptr` is valid during the lifetime of `Sampler`.
unsafe impl Send for Sampler {}
