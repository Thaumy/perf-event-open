use std::ffi::{CStr, CString};
use std::mem::align_of;

use super::{RecordId, SampleType, Task};
use crate::ffi::{bindings as b, deref_offset};

#[derive(Clone)]
pub struct Comm {
    pub record_id: Option<RecordId>,

    pub by_execve: bool,
    pub task: Task,
    pub comm: CString,
}

impl Comm {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        misc: u16,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L901
        // struct {
        //     struct perf_event_header header;
        //     u32  pid, tid;
        //     char comm[];
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let comm = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(comm.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8540
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        let by_execve = misc & b::PERF_RECORD_MISC_COMM_EXEC as u16 > 0;

        Self {
            record_id,
            by_execve,
            task,
            comm,
        }
    }
}
