use super::{RecordId, SampleType, Task};
use crate::ffi::deref_offset;

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Exit {
    pub record_id: Option<RecordId>,

    pub task: Task,
    pub parent_task: Task,
    pub time: u64,
}

impl Exit {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L912
        // struct {
        //     struct perf_event_header header;
        //     u32 pid, ppid;
        //     u32 tid, ptid;
        //     u64 time;
        //     struct sample_id sample_id;
        // };

        let pid = deref_offset(&mut ptr);
        let ppid = deref_offset(&mut ptr);
        let tid = deref_offset(&mut ptr);
        let ptid = deref_offset(&mut ptr);

        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8428
        let time = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        let task = Task { pid, tid };
        let parent_task = Task {
            pid: ppid,
            tid: ptid,
        };

        Self {
            record_id,
            task,
            parent_task,
            time,
        }
    }
}

super::from!(Exit);

super::debug!(Exit {
    {record_id?},
    {task},
    {parent_task},
    {time},
});

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fork {
    pub record_id: Option<RecordId>,

    pub task: Task,
    pub parent_task: Task,
    pub time: u64,
}

impl Fork {
    pub(crate) unsafe fn from_ptr(ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8423
        let layout = Exit::from_ptr(ptr, sample_id_all);

        Self {
            record_id: layout.record_id,
            task: layout.task,
            parent_task: layout.parent_task,
            time: layout.time,
        }
    }
}

super::from!(Fork);

super::debug!(Fork {
    {record_id?},
    {task},
    {parent_task},
    {time},
});
