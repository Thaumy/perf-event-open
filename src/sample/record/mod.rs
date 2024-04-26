use auxiliary::{Aux, AuxOutputHwId};
use bpf::BpfEvent;
use cgroup::Cgroup;
use comm::Comm;
use ctx::CtxSwitch;
use itrace::ItraceStart;
use ksymbol::Ksymbol;
use lost::{LostRecords, LostSamples};
use mmap::Mmap;
use ns::Namespaces;
use read::Read;
use sample::Sample;
use task::{Exit, Fork};
use text_poke::TextPoke;
use throttle::{Throttle, Unthrottle};

use crate::ffi::{bindings as b, deref_offset};

pub mod auxiliary;
pub mod bpf;
pub mod cgroup;
pub mod comm;
pub mod ctx;
pub mod itrace;
pub mod ksymbol;
pub mod lost;
pub mod mmap;
pub mod ns;
pub mod read;
pub mod sample;
pub mod task;
pub mod text_poke;
pub mod throttle;

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L847
#[derive(Clone)]
pub enum Record {
    // PERF_RECORD_SAMPLE
    Sample(Box<Sample>),

    // PERF_RECORD_MMAP | PERF_RECORD_MMAP2
    Mmap(Box<Mmap>),
    // PERF_RECORD_READ
    Read(Box<Read>),
    // PERF_RECORD_CGROUP
    Cgroup(Box<Cgroup>),
    // PERF_RECORD_KSYMBOL
    Ksymbol(Box<Ksymbol>),
    // PERF_RECORD_TEXT_POKE
    TextPoke(Box<TextPoke>),
    // PERF_RECORD_BPF_EVENT
    BpfEvent(Box<BpfEvent>),
    // PERF_RECORD_SWITCH | PERF_RECORD_SWITCH_CPU_WIDE
    CtxSwitch(Box<CtxSwitch>),
    // PERF_RECORD_NAMESPACES
    Namespaces(Box<Namespaces>),
    // PERF_RECORD_ITRACE_START
    ItraceStart(Box<ItraceStart>),

    // PERF_RECORD_AUX
    Aux(Box<Aux>),
    // PERF_RECORD_AUX_OUTPUT_HW_ID
    AuxOutputHwId(Box<AuxOutputHwId>),

    // PERF_RECORD_COMM
    Comm(Box<Comm>),
    // PERF_RECORD_EXIT
    Exit(Box<Exit>),
    // PERF_RECORD_FORK
    Fork(Box<Fork>),

    // PERF_RECORD_THROTTLE
    Throttle(Box<Throttle>),
    // PERF_RECORD_UNTHROTTLE
    Unthrottle(Box<Unthrottle>),

    // PERF_RECORD_LOST
    LostRecords(Box<LostRecords>),
    // PERF_RECORD_LOST_SAMPLES
    LostSamples(Box<LostSamples>),

    Unknown(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Task {
    pub pid: u32,
    pub tid: u32,
}

#[derive(Clone, Debug)]
pub enum Priv {
    // PERF_RECORD_MISC_USER
    User,
    // PERF_RECORD_MISC_KERNEL
    Kernel,
    // PERF_RECORD_MISC_HYPERVISOR
    Hv,
    // PERF_RECORD_MISC_GUEST_USER
    GuestUser,
    // PERF_RECORD_MISC_GUEST_KERNEL
    GuestKernel,
    // PERF_RECORD_MISC_CPUMODE_UNKNOWN
    Unknown,
}

impl Priv {
    pub(crate) fn from_misc(misc: u16) -> Self {
        // 3 bits
        match misc as u32 & b::PERF_RECORD_MISC_CPUMODE_MASK {
            b::PERF_RECORD_MISC_USER => Self::User,
            b::PERF_RECORD_MISC_KERNEL => Self::Kernel,
            b::PERF_RECORD_MISC_HYPERVISOR => Self::Hv,
            b::PERF_RECORD_MISC_GUEST_USER => Self::GuestUser,
            b::PERF_RECORD_MISC_GUEST_KERNEL => Self::GuestKernel,
            b::PERF_RECORD_MISC_CPUMODE_UNKNOWN => Self::Unknown,
            _ => Self::Unknown, // For compatibility, not ABI.
        }
    }
}

#[derive(Clone)]
pub struct RecordId {
    pub id: Option<u64>,
    pub stream_id: Option<u64>,
    pub cpu: Option<u32>,
    pub task: Option<Task>,
    pub time: Option<u64>,
}

pub(crate) struct SampleType(pub u64);

impl RecordId {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_type: u64) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L859
        // struct sample_id {
        //     { u32 pid, tid;  } && PERF_SAMPLE_TID
        //     { u64 time;      } && PERF_SAMPLE_TIME
        //     { u64 id;        } && PERF_SAMPLE_ID
        //     { u64 stream_id; } && PERF_SAMPLE_STREAM_ID
        //     { u32 cpu, res;  } && PERF_SAMPLE_CPU
        //     { u64 id;        } && PERF_SAMPLE_IDENTIFIER
        // } && perf_event_attr::sample_id_all

        macro_rules! when {
            ($flag:ident, $ty:ty) => {
                (sample_type & (b::$flag as u64) > 0).then(|| deref_offset::<$ty>(&mut ptr))
            };
            ($flag:ident, $then:expr) => {
                (sample_type & (b::$flag as u64) > 0).then(|| $then)
            };
        }

        let task = when!(PERF_SAMPLE_TID, {
            let pid = deref_offset(&mut ptr);
            let tid = deref_offset(&mut ptr);
            Task { pid, tid }
        });
        let time = when!(PERF_SAMPLE_TIME, u64);
        let id = when!(PERF_SAMPLE_ID, u64);
        let stream_id = when!(PERF_SAMPLE_STREAM_ID, u64);
        let cpu = when!(PERF_SAMPLE_CPU, u32);

        // For `PERF_SAMPLE_IDENTIFIER`:
        // `PERF_SAMPLE_IDENTIFIER` just duplicates the `PERF_SAMPLE_ID` at a fixed offset,
        // it's useful to distinguish the sample format if multiple events share the same rb.
        // Our design does not support redirecting samples to another rb (e.g., `PERF_FLAG_FD_OUTPUT`),
        // and this is not a parser crate, so `PERF_SAMPLE_IDENTIFIER` is not needed.
        // See:
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7342
        // https://github.com/torvalds/linux/blob/v6.13/tools/perf/Documentation/perf.data-file-format.txt#L466
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12808

        Self {
            id,
            stream_id,
            cpu,
            task,
            time,
        }
    }
}

macro_rules! from {
    ($ty:ident) => {
        impl From<Box<$ty>> for super::Record {
            fn from(value: Box<$ty>) -> Self {
                Self::$ty(value)
            }
        }
    };
}
use from;

macro_rules! debug {
    ($ty:ty { $first_field:tt, $($field:tt,)* }) => {
        impl std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use crate::sample::record::debug;

                // `{:-?}` formatter, ignores `None` fields.
                if f.sign_minus() {
                    let has_none = debug!(is_none, self, $first_field) $(|| debug!(is_none, self, $field))+;
                    write!(f, "{} {{ ", stringify!($ty))?;
                    if has_none {
                        debug!({:-?}, self, f, "{}: {:-?}, ", $first_field);
                        $(debug!({:-?}, self, f, "{}: {:-?}, ", $field);)+
                        write!(f, "..")?;
                    } else {
                        debug!({:-?}, self, f, "{}: {:-?}", $first_field);
                        $(debug!({:-?}, self, f, ", {}: {:-?}", $field);)+
                    }
                    return write!(f, " }}")
                }

                // `{:#?}` formatter, same as `{:-?}`, but with indentation.
                if f.alternate() {
                    let has_none = debug!(is_none, self, $first_field) $(|| debug!(is_none, self, $field))+;
                    let mut ds = f.debug_struct(stringify!($ty));
                    debug!({:#?}, self, ds, $first_field);
                    $(debug!({:#?}, self, ds, $field);)*
                    return if has_none {
                        ds.finish_non_exhaustive()
                    } else {
                        ds.finish()
                    }
                }

                // `{:?}` formatter, same as `#[derive(Debug)]`.
                let mut ds = f.debug_struct(stringify!($ty));
                debug!({:?}, self, ds, $first_field);
                $(debug!({:?}, self, ds, $field);)*
                ds.finish()
            }
        }
    };
    // internal switches
    (is_none, $self:ident, {$field:ident}) => {
        false
    };
    (is_none, $self:ident, {$field:ident?}) => {
        $self.$field.is_none()
    };
    ({:?}, $self:ident, $ds:ident, {$field:ident$(?)?}) => {
        $ds.field(stringify!($field), &$self.$field);
    };
    ({:#?}, $self:ident, $ds:ident, {$field:ident}) => {
        $ds.field(stringify!($field), &$self.$field);
    };
    ({:#?}, $self:ident, $ds:ident, {$field:ident?}) => {
        if let Some(it) = &$self.$field {
            $ds.field(stringify!($field), it);
        }
    };
    ({:-?}, $self:ident, $f:ident, $fmt:literal, {$field:ident}) => {
        write!($f, $fmt, stringify!($field), &$self.$field)?;
    };
    ({:-?}, $self:ident, $f:ident, $fmt:literal, {$field:ident?}) => {
        if let Some(it) = &$self.$field {
            write!($f, $fmt, stringify!($field), it)?;
        }
    };
}
pub(crate) use debug;
