use super::{RecordId, SampleType, Task};
use crate::count::Stat;
use crate::ffi::deref_offset;

#[derive(Clone)]
pub struct Read {
    pub record_id: Option<RecordId>,

    pub task: Task,
    pub stat: Stat,
}

impl Read {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        read_format: u64,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L946
        // struct {
        //     struct perf_event_header header;
        //     u32 pid, tid;
        //     struct read_format values;
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let stat = Stat::from_ptr_offset(&mut ptr, read_format);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            task,
            stat,
        }
    }
}

super::from!(Read);
