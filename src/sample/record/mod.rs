use crate::ffi::{bindings as b, deref_offset};

#[derive(Clone, Debug)]
pub struct Task {
    pub pid: u32,
    pub tid: u32,
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
