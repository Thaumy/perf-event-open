use super::{RecordId, Task};

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItraceStart {
    pub record_id: Option<RecordId>,

    pub task: Task,
}

impl ItraceStart {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1093
        // struct {
        //     struct perf_event_header header;
        //     u32 pid;
        //     u32 tid;
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let record_id = sample_id_all.map(|super::SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self { record_id, task }
    }
}

super::from!(ItraceStart);

super::debug!(ItraceStart {
    {record_id?},
    {task},
});
