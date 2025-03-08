use std::borrow::Borrow;
use std::fmt;
use std::fmt::{Debug, Formatter};

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

use super::rb::CowChunk;
use crate::ffi::{bindings as b, deref_offset, Attr};

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Record {
    // PERF_RECORD_SAMPLE
    Sample(Box<Sample>),

    // PERF_RECORD_MMAP | PERF_RECORD_MMAP2
    Mmap(Box<Mmap>),
    // PERF_RECORD_READ
    Read(Box<Read>),
    // PERF_RECORD_CGROUP
    /// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/96aaab686505c449e24d76e76507290dcc30e008>
    Cgroup(Box<Cgroup>),
    // PERF_RECORD_KSYMBOL
    /// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/76193a94522f1d4edf2447a536f3f796ce56343b>
    Ksymbol(Box<Ksymbol>),
    // PERF_RECORD_TEXT_POKE
    /// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/e17d43b93e544f5016c0251d2074c15568d5d963>
    TextPoke(Box<TextPoke>),
    // PERF_RECORD_BPF_EVENT
    /// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/6ee52e2a3fe4ea35520720736e6791df1fb67106>
    BpfEvent(Box<BpfEvent>),
    // PERF_RECORD_SWITCH | PERF_RECORD_SWITCH_CPU_WIDE
    /// Since `linux-4.3`: <https://github.com/torvalds/linux/commit/45ac1403f564f411c6a383a2448688ba8dd705a4>
    CtxSwitch(Box<CtxSwitch>),
    // PERF_RECORD_NAMESPACES
    /// Since `linux-4.12`: <https://github.com/torvalds/linux/commit/e422267322cd319e2695a535e47c5b1feeac45eb>
    Namespaces(Box<Namespaces>),
    // PERF_RECORD_ITRACE_START
    /// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/ec0d7729bbaed4b9d2d3fada693278e13a3d1368>
    ItraceStart(Box<ItraceStart>),

    // PERF_RECORD_AUX
    /// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/68db7e98c3a6ebe7284b6cf14906ed7c55f3f7f0>
    Aux(Box<Aux>),
    // PERF_RECORD_AUX_OUTPUT_HW_ID
    /// Since `linux-5.16`: <https://github.com/torvalds/linux/commit/8b8ff8cc3b8155c18162e8b1f70e1230db176862>
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
    /// Since `linux-4.2`: <https://github.com/torvalds/linux/commit/f38b0dbb491a6987e198aa6b428db8692a6480f8>
    LostSamples(Box<LostSamples>),

    Unknown(Vec<u8>),
}

impl Debug for Record {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        macro_rules! debug {
            ($($varient:ident,)+) => {
                match self {
                    $(Self::$varient(it) => {
                        if f.alternate() {
                            return write!(f, "{:#?}", it)
                        }
                        if f.sign_minus(){
                            return write!(f, "{:-?}", it)
                        }
                        write!(f, "{:?}", it)
                    })+
                }
            };
        }

        debug![
            Sample,
            Mmap,
            Read,
            Cgroup,
            Ksymbol,
            TextPoke,
            BpfEvent,
            CtxSwitch,
            Namespaces,
            ItraceStart,
            Aux,
            AuxOutputHwId,
            Comm,
            Exit,
            Fork,
            Throttle,
            Unthrottle,
            LostRecords,
            LostSamples,
            Unknown,
        ]
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Task {
    pub pid: u32,
    pub tid: u32,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RecordId {
    pub id: Option<u64>,
    pub stream_id: Option<u64>,
    pub cpu: Option<u32>,
    pub task: Option<Task>,
    pub time: Option<u64>,
}

debug!(RecordId {
    {id?},
    {stream_id?},
    {cpu?},
    {task?},
    {time?},
});

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

/// Unsafe record parser.
///
/// Unlike [`Parser`], you need to ensure the safety of parsing record bytes.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnsafeParser {
    pub sample_id_all: bool,
    pub sample_type: u64,
    pub read_format: u64,
    pub user_regs: usize,
    pub intr_regs: usize,
    pub branch_sample_type: u64,
}

impl UnsafeParser {
    pub(crate) fn from_attr(attr: &Attr) -> Self {
        Self {
            sample_id_all: attr.sample_id_all() > 0,
            sample_type: attr.sample_type,
            user_regs: attr.sample_regs_user.count_ones() as _,
            intr_regs: attr.sample_regs_intr.count_ones() as _,
            branch_sample_type: attr.branch_sample_type,
            read_format: attr.read_format,
        }
    }

    /// Parse record bytes into record type.
    ///
    /// # Safety
    ///
    /// `bytes` must be created by the same sampler as this parser.
    ///
    /// See also [`Parser`].
    pub unsafe fn parse<T>(&self, bytes: T) -> (Priv, Record)
    where
        T: Borrow<[u8]>,
    {
        let bytes = bytes.borrow();
        let ptr = &mut bytes.as_ptr();

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L824
        // struct perf_event_header {
        //     u32 type;
        //     u16 misc;
        //     u16 size;
        // };

        let ty: u32 = deref_offset(ptr);
        let misc: u16 = deref_offset(ptr);
        let record_priv = Priv::from_misc(misc);

        let ptr = ptr.add(size_of::<u16>()); // skip `size`
        let sample_id_all = self.sample_id_all.then_some(SampleType(self.sample_type));

        fn from<T>(t: T) -> Record
        where
            Box<T>: Into<Record>,
        {
            Box::new(t).into()
        }

        let record = match ty {
            b::PERF_RECORD_SAMPLE => from(Sample::from_ptr(
                ptr,
                misc,
                self.read_format,
                self.sample_type,
                self.user_regs,
                self.intr_regs,
                self.branch_sample_type,
            )),
            b::PERF_RECORD_MMAP => from(Mmap::from_ptr(ptr, misc, false, sample_id_all)),
            b::PERF_RECORD_MMAP2 => from(Mmap::from_ptr(ptr, misc, true, sample_id_all)),
            b::PERF_RECORD_READ => from(Read::from_ptr(ptr, self.read_format, sample_id_all)),
            #[cfg(feature = "linux-5.7")]
            b::PERF_RECORD_CGROUP => from(Cgroup::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-5.1")]
            b::PERF_RECORD_KSYMBOL => from(Ksymbol::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-5.9")]
            b::PERF_RECORD_TEXT_POKE => from(TextPoke::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-5.1")]
            b::PERF_RECORD_BPF_EVENT => from(BpfEvent::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-4.3")]
            b::PERF_RECORD_SWITCH => from(CtxSwitch::from_ptr(ptr, false, misc, sample_id_all)),
            #[cfg(feature = "linux-4.3")]
            b::PERF_RECORD_SWITCH_CPU_WIDE => {
                from(CtxSwitch::from_ptr(ptr, true, misc, sample_id_all))
            }
            #[cfg(feature = "linux-4.12")]
            b::PERF_RECORD_NAMESPACES => from(Namespaces::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-4.1")]
            b::PERF_RECORD_ITRACE_START => from(ItraceStart::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-4.1")]
            b::PERF_RECORD_AUX => from(Aux::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-5.16")]
            b::PERF_RECORD_AUX_OUTPUT_HW_ID => from(AuxOutputHwId::from_ptr(ptr, sample_id_all)),
            b::PERF_RECORD_COMM => from(Comm::from_ptr(ptr, misc, sample_id_all)),
            b::PERF_RECORD_EXIT => from(Exit::from_ptr(ptr, sample_id_all)),
            b::PERF_RECORD_FORK => from(Fork::from_ptr(ptr, sample_id_all)),
            b::PERF_RECORD_THROTTLE => from(Throttle::from_ptr(ptr, sample_id_all)),
            b::PERF_RECORD_UNTHROTTLE => from(Unthrottle::from_ptr(ptr, sample_id_all)),
            b::PERF_RECORD_LOST => from(LostRecords::from_ptr(ptr, sample_id_all)),
            #[cfg(feature = "linux-4.2")]
            b::PERF_RECORD_LOST_SAMPLES => from(LostSamples::from_ptr(ptr, sample_id_all)),
            _ => Record::Unknown(bytes.to_vec()), // For compatibility, not ABI.
        };

        (record_priv, record)
    }
}

/// Record parser.
///
/// This type can only be accessed within the closure scope of COW record iterators,
/// parse [`CowChunk`] with this parser is always safe since the closure scope ensures
/// that the `CowChunk` and the underlying unsafe parser are created from the same
/// sampler.
#[derive(Debug)]
pub struct Parser(pub(in crate::sample) UnsafeParser);

impl Parser {
    /// Parse [`CowChunk`] into record type.
    pub fn parse(&self, chunk: CowChunk<'_>) -> (Priv, Record) {
        let bytes = chunk.as_bytes();
        unsafe { self.0.parse(bytes) }
    }

    /// Returns the underlying unsafe record parser.
    pub fn as_unsafe(&self) -> &UnsafeParser {
        &self.0
    }
}
