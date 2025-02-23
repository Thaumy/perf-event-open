use std::io::Result;

use crate::ffi::bindings as b;

pub(super) mod attr;
pub mod sibling;
mod target;

pub use target::*;

macro_rules! unsupported {
    () => {
        Err(std::io::ErrorKind::Unsupported)?
    };
    ($bool:expr) => {
        if $bool {
            Err(std::io::ErrorKind::Unsupported)?
        }
    };
}
pub(super) use unsupported;

#[derive(Clone, Debug, Default)]
pub struct Opts {
    pub exclude: Priv,
    pub only_group: bool,
    pub pin_on_pmu: bool,

    // `mmap` will fail if this option is used with all CPUs:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L6575
    pub inherit: Option<Inherit>,
    pub on_execve: Option<OnExecve>,
    pub stat_format: StatFormat,

    pub enable: bool,
    pub sample_on: SampleOn,
    pub sample_skid: SampleSkid,
    pub sample_format: SampleFormat,
    pub extra_record: ExtraRecord,
    pub record_id_all: bool,
    pub record_id_format: RecordIdFormat,
    pub wake_up: WakeUp,
    // Must be used together with `remove_on_exec`:
    // https://github.com/torvalds/linux/blob/2408a807bfc3f738850ef5ad5e3fd59d66168996/kernel/events/core.c#L12582
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/97ba62b278674293762c3d91f724f1bb922f04e0>
    pub sigtrap_on_sample: Option<SigData>,
    /// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/34f439278cef7b1177f8ce24f9fc81dfc6221d3b>
    pub timer: Option<Clock>,
    /// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
    pub pause_aux: bool,
}

/// Privilege levels.
#[derive(Clone, Debug, Default)]
pub struct Priv {
    /// User space.
    pub user: bool,

    /// Kernel space.
    pub kernel: bool,

    /// Hypervisor.
    pub hv: bool,

    /// Host mode.
    pub host: bool,

    /// Guest mode.
    pub guest: bool,

    /// Idle task.
    pub idle: bool,
}

/// Controls the inherit behavior.
#[derive(Clone, Debug)]
pub enum Inherit {
    /// New child tasks will inherit the counter.
    ///
    /// This indicates if the process we monitoring creates new tasks
    /// (child process or thread), the counter will count events that
    /// happens in these tasks.
    ///
    /// This applies only to new children, not to any existing children at the time
    /// the counter is created (nor to any new children of existing children).
    NewChild,

    /// Same as [`NewChild`][Self::NewChild], but only new threads will inherit the counter.
    ///
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/2b26f0aa004995f49f7b6f4100dd0e4c39a9ed5f>
    NewThread,
}

// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12535
/// Counter behavior when calling [`execve`](https://man7.org/linux/man-pages/man2/execve.2.html).
#[derive(Clone, Debug)]
pub enum OnExecve {
    /// Enable counter.
    Enable,

    /// Remove counter.
    ///
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/2e498d0a74e5b88a6689ae1b811f247f91ff188e>
    Remove,
}

/// Controls the format of [`Stat`][crate::count::Stat].
#[derive(Clone, Debug, Default)]
pub struct StatFormat {
    /// Contains the [event ID][crate::count::SiblingStat::id].
    pub id: bool,

    /// Contains the [enabled time][crate::count::Stat::time_enabled] of the counter.
    pub time_enabled: bool,

    /// Contains the [running time][crate::count::Stat::time_running] of the counter.
    pub time_running: bool,

    /// Contains the [number of lost records][crate::count::SiblingStat::lost_records].
    ///
    /// Since `linux-6.0`: <https://github.com/torvalds/linux/commit/119a784c81270eb88e573174ed2209225d646656>
    pub lost_records: bool,

    /// Contains [sibling event counts][crate::count::Stat::siblings].
    pub siblings: bool,
}

impl StatFormat {
    pub(crate) fn as_read_format(&self) -> Result<u64> {
        let mut val = 0;
        macro_rules! when {
            ($field:ident, $flag:ident) => {
                if self.$field {
                    val |= b::$flag;
                }
            };
        }
        when!(id, PERF_FORMAT_ID);
        when!(time_enabled, PERF_FORMAT_TOTAL_TIME_ENABLED);
        when!(time_running, PERF_FORMAT_TOTAL_TIME_RUNNING);
        #[cfg(feature = "linux-6.0")]
        when!(lost_records, PERF_FORMAT_LOST);
        #[cfg(not(feature = "linux-6.0"))]
        unsupported!(self.lost_records);
        when!(siblings, PERF_FORMAT_GROUP);
        Ok(val as _)
    }
}

// Details about overflow:
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9958
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L5944
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L10036
/// Controls when to generate a [sample record][crate::sample::record::sample::Sample].
///
/// Defaults to `Count(0)` (no sample mode in `perf record` command),
/// set it to the desired rate to generate sample records.
///
/// The maximum sample rate is specified in `/proc/sys/kernel/perf_event_max_sample_rate`,
/// [`Throttle`][crate::sample::record::throttle::Throttle] record will be generated if
/// the limit has been reached.
///
/// Meanwhile, `/proc/sys/kernel/perf_cpu_time_max_percent` limits the CPU time allowed
/// to handle sampling (0 means unlimited). Sampling also will be throttled if this limit
/// has been reached.
///
/// # Event overflow
///
/// The kernel maintains an unsigned counter with an appropriate negative initial value,
/// which will finally overflows since every event increase it by one. Then sampling will
/// be triggered and that counter will be reset to prepare for the next overflow. This is
/// what this option actually controls.
///
/// In addition to asynchronous iterators with [wake up][WakeUp] option, overflow can
/// also be captured by enabling I/O signaling from the perf event fd, which indicates
/// `POLL_IN` on each overflow.
///
/// Here is an example:
///
/// ```rust
/// // Fork to avoid signal handler conflicts.
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
/// use std::mem::MaybeUninit;
/// use std::os::fd::AsRawFd;
/// use std::ptr::null_mut;
/// use std::sync::atomic::AtomicBool;
/// use std::sync::atomic::Ordering;
///
/// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// static IN: AtomicBool = AtomicBool::new(false);
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
///     // that binding (same as `POLL_IN` below).
///     libc::fcntl(fd, 10, libc::SIGIO);
///     libc::fcntl(fd, libc::F_SETOWN, libc::getpid());
/// }
///
/// fn handler(num: i32, info: *const libc::siginfo_t) {
///     assert_eq!(num, libc::SIGIO);
///     let si_code = unsafe { *info }.si_code;
///     assert_eq!(si_code, 1); // POLL_IN
///     IN.store(true, Ordering::Relaxed);
/// }
/// let act = libc::sigaction {
///     sa_sigaction: handler as _,
///     sa_mask: unsafe { MaybeUninit::zeroed().assume_init() },
///     sa_flags: libc::SA_SIGINFO,
///     sa_restorer: None,
/// };
/// unsafe { libc::sigaction(libc::SIGIO, &act as _, null_mut()) };
///
/// let sampler = counter.sampler(5).unwrap();
/// counter.enable().unwrap();
///
/// while !IN.load(Ordering::Relaxed) {
///     std::hint::spin_loop();
/// }
///
/// println!("{:-?}", sampler.iter().next());
/// # });
/// # if result.is_err() {
/// #     unsafe { libc::abort() };
/// # }
///
/// # unsafe { libc::exit(0) };
/// ```
///
/// For more information on I/O signals, see also
/// [`Sampler::enable_counter_with`][crate::sample::Sampler::enable_counter_with].
#[derive(Clone, Debug)]
pub enum SampleOn {
    /// Sample on frequency (Hz).
    ///
    /// The kernel will adjust the sampling period to try and achieve the desired rate.
    ///
    /// `Freq(0)` means no overflow, i.e., sample records will never be generated.
    Freq(u64),

    /// Sample on every N event counts.
    ///
    /// It is referred to sample period.
    ///
    /// `Count(0)` is the default value for `SampleOn`, it has the same meaning as `Freq(0)`.
    Count(u64),
}

impl Default for SampleOn {
    fn default() -> Self {
        Self::Freq(0)
    }
}

/// Controls the amount of sample skid.
///
/// Skid is how many instructions execute between an event of interest happening and
/// the kernel being able to stop and record the event.
///
/// Smaller skid is better and allows more accurate reporting of which events correspond
/// to which instructions, but hardware is often limited with how small this can be.
///
/// This affects the precision of [`code_addr`][crate::sample::record::sample::Sample::code_addr].
#[derive(Clone, Debug)]
pub enum SampleSkid {
    /// Can have arbitrary skid.
    Arbitrary,
    /// Must have constant skid.
    Const,
    /// Requested to have 0 skid.
    ReqZero,
    /// Must have 0 skid.
    Zero,
}

impl SampleSkid {
    pub(crate) fn as_precise_ip(&self) -> u8 {
        match self {
            Self::Arbitrary => 0,
            Self::Const => 1,
            Self::ReqZero => 2,
            Self::Zero => 3,
        }
    }
}

impl Default for SampleSkid {
    fn default() -> Self {
        Self::Arbitrary
    }
}

/// Controls the format of [sample record][crate::sample::record::sample::Sample].
#[derive(Clone, Debug, Default)]
pub struct SampleFormat {
    // PERF_SAMPLE_READ
    /// Contains [counter statistics][crate::sample::record::sample::Sample::stat].
    pub stat: bool,
    // PERF_SAMPLE_PERIOD
    /// Contains [sample period][crate::sample::record::sample::Sample::period].
    pub period: bool,
    // PERF_SAMPLE_CGROUP
    /// Contains [cgroup info][crate::sample::record::sample::Sample::cgroup].
    ///
    /// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/6546b19f95acc986807de981402bbac6b3a94b0f>
    pub cgroup: bool,
    // PERF_SAMPLE_CALLCHAIN
    /// Contains [call chain][crate::sample::record::sample::Sample::call_chain].
    pub call_chain: Option<CallChain>,
    // PERF_SAMPLE_STACK_USER
    /// Contains [user stack][crate::sample::record::sample::Sample::user_stack].
    pub user_stack: Option<Size>,

    // PERF_SAMPLE_ADDR
    /// Contains [data address][crate::sample::record::sample::Sample::data_addr].
    pub data_addr: bool,
    // PERF_SAMPLE_PHYS_ADDR
    /// Contains [physical data address][crate::sample::record::sample::Sample::data_phys_addr].
    ///
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/fc7ce9c74c3ad232b084d80148654f926d01ece7>
    pub data_phys_addr: bool,
    // PERF_SAMPLE_DATA_PAGE_SIZE
    /// Contains [data page size][crate::sample::record::sample::Sample::data_page_size].
    ///
    /// Since `linux-5.11`: <https://github.com/torvalds/linux/commit/8d97e71811aaafe4abf611dc24822fd6e73df1a1>
    pub data_page_size: bool,
    // PERF_SAMPLE_DATA_SRC
    /// Contains [data source][crate::sample::record::sample::Sample::data_source].
    pub data_source: bool,

    // PERF_SAMPLE_IP
    /// Contains [code address][crate::sample::record::sample::Sample::code_addr].
    pub code_addr: bool,
    // PERF_SAMPLE_CODE_PAGE_SIZE
    /// Contains [code page size][crate::sample::record::sample::Sample::code_page_size].
    ///
    /// Since `linux-5.11`: <https://github.com/torvalds/linux/commit/995f088efebe1eba0282a6ffa12411b37f8990c2>
    pub code_page_size: bool,

    // PERF_SAMPLE_REGS_USER
    /// Contains [user level registers][crate::sample::record::sample::Sample::user_regs].
    pub user_regs: Option<RegsMask>,
    // PERF_SAMPLE_REGS_INTR
    /// Contains [registers on interrupt][crate::sample::record::sample::Sample::intr_regs].
    pub intr_regs: Option<RegsMask>,

    // PERF_SAMPLE_RAW
    /// Contains [raw data][crate::sample::record::sample::Sample::raw].
    pub raw: bool,
    // PERF_SAMPLE_BRANCH_STACK
    /// Contains [LBR data][crate::sample::record::sample::Sample::lbr].
    pub lbr: Option<Lbr>,
    // PERF_SAMPLE_AUX
    /// Contains [AUX area snapshot][crate::sample::record::sample::Sample::aux].
    ///
    /// Since `linux-5.5`: <https://github.com/torvalds/linux/commit/a4faf00d994c40e64f656805ac375c65e324eefb>
    pub aux: Option<Size>,
    // PERF_SAMPLE_TRANSACTION
    /// Contains [the sources of any transactional memory aborts][crate::sample::record::sample::Sample::txn].
    pub txn: bool,
    // PERF_SAMPLE_WEIGHT / PERF_SAMPLE_WEIGHT_STRUCT
    /// Contains [sample weight][crate::sample::record::sample::Sample::weight].
    pub weight: Option<Repr>,
}

#[derive(Clone, Debug, Default)]
pub struct Lbr {
    // Inherit exclude_{kernel, user, hv} from attr if not set:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12473
    pub target_priv: Option<TargetPriv>,
    pub branch_type: BranchType,
    // PERF_SAMPLE_BRANCH_HW_INDEX
    /// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/bbfd5e4fab63703375eafaf241a0c696024a59e1>
    pub hw_index: bool,
    pub entry_format: EntryFormat,
}

#[derive(Clone, Debug)]
pub struct TargetPriv {
    // PERF_SAMPLE_BRANCH_USER
    pub user: bool,
    // PERF_SAMPLE_BRANCH_KERNEL
    pub kernel: bool,
    // PERF_SAMPLE_BRANCH_HV
    pub hv: bool,
}

impl TargetPriv {
    #[rustfmt::skip]
    pub(crate) fn as_branch_sample_type(&self) -> u64 {
        let u = if self.user { b::PERF_SAMPLE_BRANCH_USER } else { 0 };
        let k = if self.kernel { b::PERF_SAMPLE_BRANCH_KERNEL } else { 0 };
        let h = if self.hv { b::PERF_SAMPLE_BRANCH_HV } else { 0 };
        ( u | k | h ) as _
    }
}

#[derive(Clone, Debug, Default)]
pub struct BranchType {
    // PERF_SAMPLE_BRANCH_ANY
    pub any: bool,
    // PERF_SAMPLE_BRANCH_ANY_RETURN
    pub any_return: bool,
    // PERF_SAMPLE_BRANCH_COND
    pub cond: bool,
    // PERF_SAMPLE_BRANCH_IND_JUMP
    /// Since `linux-4.2`: <https://github.com/torvalds/linux/commit/c9fdfa14c3792c0160849c484e83aa57afd80ccc>
    pub ind_jump: bool,
    // PERF_SAMPLE_BRANCH_CALL_STACK
    /// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/2c44b1936bb3b135a3fac8b3493394d42e51cf70>
    pub call_stack: bool,

    // PERF_SAMPLE_BRANCH_CALL
    /// Since `linux-4.4`: <https://github.com/torvalds/linux/commit/c229bf9dc179d2023e185c0f705bdf68484c1e73>
    pub call: bool,
    // PERF_SAMPLE_BRANCH_IND_CALL
    pub ind_call: bool,
    // PERF_SAMPLE_BRANCH_ANY_CALL
    pub any_call: bool,

    // PERF_SAMPLE_BRANCH_IN_TX
    pub in_tx: bool,
    // PERF_SAMPLE_BRANCH_NO_TX
    pub no_tx: bool,
    // PERF_SAMPLE_BRANCH_ABORT_TX
    pub abort_tx: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EntryFormat {
    // PERF_SAMPLE_BRANCH_NO_FLAGS
    /// Must be enabled before `linux-4.5`:
    /// <https://github.com/torvalds/linux/commit/b16a5b52eb90d92b597257778e51e1fdc6423e64>
    pub flags: bool,
    // PERF_SAMPLE_BRANCH_NO_CYCLES
    /// Must be enabled before `linux-4.5`:
    /// <https://github.com/torvalds/linux/commit/b16a5b52eb90d92b597257778e51e1fdc6423e64>
    pub cycles: bool,
    // PERF_SAMPLE_BRANCH_COUNTERS
    /// Since `linux-6.8`: <https://github.com/torvalds/linux/commit/571d91dcadfa3cef499010b4eddb9b58b0da4d24>
    pub counter: bool,

    // PERF_SAMPLE_BRANCH_TYPE_SAVE
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/eb0baf8a0d9259d168523b8e7c436b55ade7c546>
    pub branch_type: bool,
    // PERF_SAMPLE_BRANCH_PRIV_SAVE
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/5402d25aa5710d240040f73fb13d7d5c303ef071>
    pub branch_priv: bool,
}

#[derive(Clone, Debug)]
pub struct Size(pub u32);

/// Controls how weight values are represented.
#[derive(Clone, Debug)]
pub enum Repr {
    // PERF_SAMPLE_WEIGHT
    /// Represent weight value as [`Full`][crate::sample::record::sample::Weight::Full].
    Full,

    // PERF_SAMPLE_WEIGHT_STRUCT
    /// Represent weight value as [`Vars`][crate::sample::record::sample::Weight::Vars].
    ///
    /// Since`linux-5.12`: <https://github.com/torvalds/linux/commit/2a6c6b7d7ad346f0679d0963cb19b3f0ea7ef32c>
    Vars,
}

/// Call chain options.
#[derive(Clone, Debug)]
pub struct CallChain {
    /// Exclude user call chains.
    pub exclude_user: bool,

    /// Exclude kernel call chains.
    pub exclude_kernel: bool,

    /// How many stack frames to report when generating the call chain.
    ///
    /// The maximum frames is specified in `/proc/sys/kernel/perf_event_max_stack`.
    ///
    /// Since `linux-4.8`: <https://github.com/torvalds/linux/commit/97c79a38cd454602645f0470ffb444b3b75ce574>
    pub max_stack_frames: u16,
}

/// Register mask that defines the set of CPU registers to dump on samples.
///
/// The layout of the register mask is architecture-specific and is described
/// in the kernel header file `arch/<arch>/include/uapi/asm/perf_regs.h`.
#[derive(Clone, Debug)]
pub struct RegsMask(pub u64);

/// Generate extra record types.
#[derive(Clone, Debug, Default)]
pub struct ExtraRecord {
    /// Generate [`Fork`][crate::sample::record::task::Fork]
    /// and [`Exit`][crate::sample::record::task::Exit] records.
    pub task: bool,

    /// Generate [`Read`][crate::sample::record::read::Read] records.
    ///
    /// Only meaningful if [`inherit`][Opts::inherit] is enabled.
    pub read: bool,

    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    /// Generate [`Comm`][crate::sample::record::comm::Comm] records.
    ///
    /// This also enables [`Self::task`].
    pub comm: bool,

    /// [`Mmap`][crate::sample::record::mmap::Mmap] record options.
    pub mmap: Mmap,

    /// Generate [`Cgroup`][crate::sample::record::cgroup::Cgroup] records.
    ///
    /// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/96aaab686505c449e24d76e76507290dcc30e008>
    pub cgroup: bool,

    /// Generate [`Ksymbol`][crate::sample::record::ksymbol::Ksymbol] records.
    ///
    /// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/76193a94522f1d4edf2447a536f3f796ce56343b>
    pub ksymbol: bool,

    /// Generate [`BpfEvent`][crate::sample::record::bpf::BpfEvent] records.
    ///
    /// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/6ee52e2a3fe4ea35520720736e6791df1fb67106>
    pub bpf_event: bool,

    /// Generate [`TextPoke`][crate::sample::record::text_poke::TextPoke] records.
    ///
    /// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/e17d43b93e544f5016c0251d2074c15568d5d963>
    pub text_poke: bool,

    /// Generate [`CtxSwitch`][crate::sample::record::ctx::CtxSwitch] records.
    ///
    /// Since `linux-4.3`: <https://github.com/torvalds/linux/commit/45ac1403f564f411c6a383a2448688ba8dd705a4>
    pub ctx_switch: bool,

    /// Generate [`Namespaces`][crate::sample::record::ns::Namespaces] records.
    ///
    /// Since `linux-4.12`: <https://github.com/torvalds/linux/commit/e422267322cd319e2695a535e47c5b1feeac45eb>
    pub namespaces: bool,
}

/// Controls the format of [`RecordId`][crate::sample::record::RecordId].
#[derive(Clone, Debug, Default)]
pub struct RecordIdFormat {
    // PERF_SAMPLE_ID
    /// Contains [event ID][crate::sample::record::RecordId::id].
    pub id: bool,

    // PERF_SAMPLE_STREAM_ID
    /// Contains [event stream ID][crate::sample::record::RecordId::stream_id].
    pub stream_id: bool,

    // PERF_SAMPLE_CPU
    /// Contains [CPU number][crate::sample::record::RecordId::cpu].
    pub cpu: bool,

    // PERF_SAMPLE_TID
    /// Contains [task info][crate::sample::record::RecordId::task].
    pub task: bool,

    // PERF_SAMPLE_TIME
    /// Contains [timestamp][crate::sample::record::RecordId::time].
    pub time: bool,
}

/// Wake up options for asynchronous iterators.
#[derive(Clone, Debug, Default)]
pub struct WakeUp {
    /// When to wake up asynchronous iterators.
    pub on: WakeUpOn,

    /// Wake up asynchronous iterators on every N bytes available in the AUX area.
    ///
    /// `0` means never wake up.
    ///
    /// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/1a5941312414c71dece6717da9a0fa1303127afa>
    pub on_aux_bytes: u32,
}

/// When to wake up asynchronous iterators.
///
/// "wake up" means notifying the async runtime to schedule the
/// asynchronous iterator's future to be pulled in the next round.
///
/// For performance reasons, we may not want to wake up asynchronous
/// iterators as soon as data is available. With this option we can
/// configure the number of bytes or samples that triggers the wake
/// up.
///
/// If we specify the [`Proc`] instead of [`All`], asynchronous iterators
/// will be woken up when the target process exits.
///
/// # Examples
///
/// ```rust
/// # tokio_test::block_on(async {
/// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, WakeUpOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::TaskClock;
/// let target = (Proc::ALL, Cpu(0));
///
/// let mut opts = Opts::default();
/// opts.sample_on = SampleOn::Freq(1000);
/// // Wake up asynchronous iterators on every sample.
/// opts.wake_up.on = WakeUpOn::Samples(1);
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// let mut iter = sampler.iter().into_async().unwrap();
/// println!("{:-?}", iter.next().await);
/// # });
/// ```
#[derive(Clone, Debug)]
pub enum WakeUpOn {
    /// Wake up on every N bytes available.
    ///
    /// `Bytes(0)` means never wake up.
    Bytes(u64),

    /// Wake up on every N samples available.
    ///
    /// `Samples(0)` means never wake up.
    Samples(u64),
}

impl Default for WakeUpOn {
    fn default() -> Self {
        Self::Samples(0)
    }
}

/// Semantic wrapper for signal data to pass.
///
/// This data will be copied to user's signal handler (through `si_perf`
/// in the `siginfo_t`) to disambiguate which event triggered the signal.
///
/// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/97ba62b278674293762c3d91f724f1bb922f04e0>
#[derive(Clone, Debug)]
pub struct SigData(pub u64);

/// Available internal Linux timers.
#[derive(Clone, Debug)]
pub enum Clock {
    // CLOCK_TAI
    /// A nonsettable system-wide clock derived from wall-clock time but ignoring leap seconds.
    ///
    /// This clock does not experience discontinuities and backwards jumps caused by
    /// NTP inserting leap seconds as [`RealTime`][Self::RealTime] does.
    ///
    /// This is International Atomic Time (TAI).
    Tai,

    // CLOCK_REALTIME
    /// A settable system-wide clock that measures real (i.e., wall-clock) time.
    ///
    /// This clock is affected by discontinuous jumps in the system time
    /// (e.g., if the system administrator manually changes the clock),
    /// and by the incremental adjustments performed by
    /// [`adjtime`](https://www.man7.org/linux/man-pages/man3/adjtime.3.html) and NTP.
    RealTime,

    // CLOCK_BOOTTIME
    /// Similar to [`Monotonic`][Self::Monotonic], but it also includes
    /// any time that the system is suspended.
    BootTime,

    // CLOCK_MONOTONIC
    /// A nonsettable system-wide clock that represents monotonic time since
    /// the system booted.
    ///
    /// This clock is not affected by discontinuous jumps in the system time
    /// (e.g., if the system administrator manually changes the clock), but
    /// is affected by the incremental adjustments performed by
    /// [`adjtime`](https://www.man7.org/linux/man-pages/man3/adjtime.3.html) and NTP.
    ///
    /// This time never go backwards, but successive calls may return identical
    /// (not-increased) time values.
    ///
    /// This clock does not count time that the system is suspended.
    Monotonic,

    // CLOCK_MONOTONIC_RAW
    /// Similar to [`Monotonic`][Self::Monotonic], but provides access to a
    /// raw hardware-based time that is not subject to NTP adjustments or the
    /// incremental adjustments performed by [`adjtime`](https://www.man7.org/linux/man-pages/man3/adjtime.3.html).
    MonotonicRaw,
}

/// [`Mmap`][crate::sample::record::mmap::Mmap] record options.
#[derive(Clone, Debug, Default)]
pub struct Mmap {
    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    /// Generate [`Mmap`] record when mmap is executable (with `PROT_EXEC`).
    ///
    /// This also enables [`ExtraRecord::task`].
    pub code: bool,

    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    /// Generate [`Mmap`] record when mmap is non-executable (without `PROT_EXEC`).
    ///
    /// This also enables [`ExtraRecord::task`].
    pub data: bool,

    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    /// Enable [extension fields][crate::sample::record::mmap::Mmap::ext].
    ///
    /// This also enables [`Self::code`] and [`ExtraRecord::task`].
    pub ext: Option<UseBuildId>,
}

/// Carry [`BuildId`][crate::sample::record::mmap::Info::BuildId] instead of
/// [`Device`][crate::sample::record::mmap::Info::Device] in [`Mmap`][crate::sample::record::mmap::Mmap] records
/// if possible.
///
/// The Build ID is carried if memory is mapped to an ELF file containing
/// a Build ID. Otherwise, device info is used as a fallback.
///
/// Since `linux-5.12`: <https://github.com/torvalds/linux/commit/88a16a1309333e43d328621ece3e9fa37027e8eb>
#[derive(Clone, Debug, Default)]
pub struct UseBuildId(pub bool);

/*
EventConifg::ty           u32 type_
size_of::<Attr>           u32 size
EventConifg::config       u64 config
SampleOn                  u64 __bindgen_anon_1    sample method union
{Sample, RecordId}Format  u64 sample_type
Opts::stat_format         u64 read_format
-                         ZST _bitfield_align_1
(See below)               u64 _bitfield_1         option bits
WakeUpOn                  u32 __bindgen_anon_2    wakeup on union
Breakpoint::ty            u32 bp_type
EventConifg::config1      u64 __bindgen_anon_3    config1 union
EventConifg::config2      u64 __bindgen_anon_4    config2 union
Lbr                       u64 branch_sample_type
SampleFormat::user_regs   u64 sample_regs_user
SampleFormat::user_stack  u32 sample_stack_user
Clock                     i32 clockid
SampleFormat::intr_regs   u64 sample_regs_intr
WakeUp::on_aux_bytes      u32 aux_watermark
SampleFormat::call_chain  u16 sample_max_stack
-                         u16 __reserved_2
SampleFormat::aux         u32 aux_sample_size
(See below)               u32 __bindgen_anon_5    aux action bits
SigData                   u64 sig_data
EventConfig::config3      u64 config3
*/

/*
Opts::auto_start           1 disabled                  off by default
Opts::inherit              1 inherit                   children inherit it
Opts::pin_on_pmu           1 pinned                    must always be on PMU
Opts::only_group           1 exclusive                 only group on PMU
Priv::user                 1 exclude_user              don't count user
Priv::kernel               1 exclude_kernel            ditto kernel
Priv::hv                   1 exclude_hv                ditto hypervisor
Priv::idle                 1 exclude_idle              don't count when idle
ExtraRecord::mmap          1 mmap                      include mmap data
ExtraRecord::comm          1 comm                      include comm data
SampleOn::Freq             1 freq                      use freq, not period
ExtraRecord::read          1 inherit_stat              per task counts
Opts::on_execve            1 enable_on_exec            next exec enables
ExtraRecord::task          1 task                      trace fork/exit
WakeUpOn                   1 watermark                 wakeup_watermark
IpSkid                     2 precise_ip                skid constraint
ExtraRecord::mmap          1 mmap_data                 non-exec mmap data
Opts::record_id_all        1 sample_id_all             sample_type all events
Priv::host                 1 exclude_host              don't count in host
Priv::guest                1 exclude_guest             don't count in guest
SampleFormat::call_chain   1 exclude_callchain_kernel  exclude kernel callchains
SampleFormat::call_chain   1 exclude_callchain_user    exclude user callchains
ExtraRecord::mmap          1 mmap2                     include mmap with inode data
false                      1 comm_exec                 flag comm events that are due to an exec
Opts::clock_time           1 use_clockid               use @clockid for time fields
ExtraRecord::ctx_switch    1 context_switch            context switch data
false                      1 write_backward            Write ring-buffer from end to beginning
ExtraRecord::namespaces    1 namespaces                include namespaces data
ExtraRecord::ksymbol       1 ksymbol                   include ksymbol events
ExtraRecord::bpf_event     1 bpf_event                 include bpf events
sibling::Opts::aux_output  1 aux_output                generate AUX records instead of events
ExtraRecord::cgroup        1 cgroup                    include cgroup events
ExtraRecord::text_poke     1 text_poke                 include text poke events
ExtraRecord::mmap          1 build_id                  use build id in mmap2 events
Inherit                    1 inherit_thread            children only inherit if cloned with CLONE_THREAD
Opts::on_execve            1 remove_on_exec            event is removed from task on exec
Opts::sigtrap_on_sample    1 sigtrap                   send synchronous SIGTRAP on event
-                         26 __reserved_1
*/

/*
Opts::pause_aux     1 aux_start_paused   start AUX area tracing paused
sibling::AuxTracer  1 aux_pause          on overflow, pause AUX area tracing
sibling::AuxTracer  1 aux_resume         on overflow, resume AUX area tracing
-                  29	__reserved_3
*/
