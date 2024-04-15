use crate::ffi::{bindings as b, deref_offset};

pub mod comm;

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
