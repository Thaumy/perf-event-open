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
    pub timer: Option<Clock>,
    /// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
    pub pause_aux: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Priv {
    pub user: bool,
    pub kernel: bool,
    pub hv: bool,
    pub host: bool,
    pub guest: bool,
    pub idle: bool,
}

#[derive(Clone, Debug)]
pub enum Inherit {
    NewChild,
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/2b26f0aa004995f49f7b6f4100dd0e4c39a9ed5f>
    NewThread,
}

// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12535
#[derive(Clone, Debug)]
pub enum OnExecve {
    Enable,
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/2e498d0a74e5b88a6689ae1b811f247f91ff188e>
    Remove,
}

#[derive(Clone, Debug, Default)]
pub struct StatFormat {
    pub id: bool,
    pub time_enabled: bool,
    pub time_running: bool,
    /// Since `linux-6.0`: <https://github.com/torvalds/linux/commit/119a784c81270eb88e573174ed2209225d646656>
    pub lost_records: bool,
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
#[derive(Clone, Debug)]
pub enum SampleOn {
    Freq(u64),
    Count(u64),
}

impl Default for SampleOn {
    fn default() -> Self {
        Self::Freq(0)
    }
}

#[derive(Clone, Debug)]
pub enum SampleSkid {
    Arbitrary,
    Const,
    ReqZero,
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

#[derive(Clone, Debug, Default)]
pub struct SampleFormat {
    // PERF_SAMPLE_READ
    pub stat: bool,
    // PERF_SAMPLE_PERIOD
    pub period: bool,
    // PERF_SAMPLE_CGROUP
    pub cgroup: bool,
    // PERF_SAMPLE_CALLCHAIN
    pub call_chain: Option<CallChain>,
    // PERF_SAMPLE_STACK_USER
    pub user_stack: Option<Size>,

    // PERF_SAMPLE_ADDR
    pub data_addr: bool,
    // PERF_SAMPLE_PHYS_ADDR
    pub data_phys_addr: bool,
    // PERF_SAMPLE_DATA_PAGE_SIZE
    pub data_page_size: bool,
    // PERF_SAMPLE_DATA_SRC
    pub data_source: bool,

    // PERF_SAMPLE_IP
    pub code_addr: bool,
    // PERF_SAMPLE_CODE_PAGE_SIZE
    pub code_page_size: bool,

    // PERF_SAMPLE_REGS_USER
    pub user_regs: Option<RegsMask>,
    // PERF_SAMPLE_REGS_INTR
    pub intr_regs: Option<RegsMask>,

    // PERF_SAMPLE_RAW
    pub raw: bool,
    // PERF_SAMPLE_BRANCH_STACK
    pub lbr: Option<Lbr>,
    // PERF_SAMPLE_AUX
    pub aux: Option<Size>,
    // PERF_SAMPLE_TRANSACTION
    pub txn: bool,
    // PERF_SAMPLE_WEIGHT / PERF_SAMPLE_WEIGHT_STRUCT
    pub weight: Option<Repr>,
}

#[derive(Clone, Debug, Default)]
pub struct Lbr {
    // Inherit exclude_{kernel, user, hv} from attr if not set:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12473
    pub target_priv: Option<TargetPriv>,
    pub branch_type: BranchType,
    // PERF_SAMPLE_BRANCH_HW_INDEX
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
    pub ind_jump: bool,
    // PERF_SAMPLE_BRANCH_CALL_STACK
    pub call_stack: bool,

    // PERF_SAMPLE_BRANCH_CALL
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
    pub flags: bool,
    // PERF_SAMPLE_BRANCH_NO_CYCLES
    pub cycles: bool,
    // PERF_SAMPLE_BRANCH_COUNTERS
    /// Since `linux-6.8`: <https://github.com/torvalds/linux/commit/571d91dcadfa3cef499010b4eddb9b58b0da4d24>
    pub counter: bool,

    // PERF_SAMPLE_BRANCH_TYPE_SAVE
    pub branch_type: bool,
    // PERF_SAMPLE_BRANCH_PRIV_SAVE
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/5402d25aa5710d240040f73fb13d7d5c303ef071>
    pub branch_priv: bool,
}

#[derive(Clone, Debug)]
pub struct Size(pub u32);

#[derive(Clone, Debug)]
pub enum Repr {
    // PERF_SAMPLE_WEIGHT
    Full,
    // PERF_SAMPLE_WEIGHT_STRUCT
    /// Since`linux-5.12`: <https://github.com/torvalds/linux/commit/2a6c6b7d7ad346f0679d0963cb19b3f0ea7ef32c>
    Vars,
}

#[derive(Clone, Debug)]
pub struct CallChain {
    pub exclude_user: bool,
    pub exclude_kernel: bool,
    pub max_stack_frames: u16,
}

#[derive(Clone, Debug)]
pub struct RegsMask(pub u64);

#[derive(Clone, Debug, Default)]
pub struct ExtraRecord {
    pub task: bool,
    pub read: bool,
    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    pub comm: bool,
    pub mmap: Mmap,
    pub cgroup: bool,
    pub ksymbol: bool,
    pub bpf_event: bool,
    pub text_poke: bool,
    pub ctx_switch: bool,
    pub namespaces: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RecordIdFormat {
    // PERF_SAMPLE_ID
    pub id: bool,
    // PERF_SAMPLE_STREAM_ID
    pub stream_id: bool,
    // PERF_SAMPLE_CPU
    pub cpu: bool,
    // PERF_SAMPLE_TID
    pub task: bool,
    // PERF_SAMPLE_TIME
    pub time: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WakeUp {
    pub on: WakeUpOn,
    pub on_aux_bytes: u32,
}

#[derive(Clone, Debug)]
pub enum WakeUpOn {
    Bytes(u64),
    Samples(u64),
}

impl Default for WakeUpOn {
    fn default() -> Self {
        Self::Samples(0)
    }
}

/// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/97ba62b278674293762c3d91f724f1bb922f04e0>
#[derive(Clone, Debug)]
pub struct SigData(pub u64);

#[derive(Clone, Debug)]
pub enum Clock {
    // CLOCK_TAI
    Tai,
    // CLOCK_REALTIME
    RealTime,
    // CLOCK_BOOTTIME
    BootTime,
    // CLOCK_MONOTONIC
    Monotonic,
    // CLOCK_MONOTONIC_RAW
    MonotonicRaw,
}

#[derive(Clone, Debug, Default)]
pub struct Mmap {
    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    pub code: bool,

    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    pub data: bool,

    // This also enables `task`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8389
    pub ext: Option<UseBuildId>,
}

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
