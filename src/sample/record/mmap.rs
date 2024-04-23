use std::ffi::{CStr, CString};
use std::mem::align_of;

use super::{RecordId, SampleType, Task};
use crate::ffi::{bindings as b, deref_offset};

#[derive(Clone)]
pub struct Mmap {
    pub record_id: Option<RecordId>,

    pub executable: bool,
    pub task: Task,
    pub addr: u64,
    pub len: u64,
    pub file: CString,
    pub page_offset: u64,
}

impl Mmap {
    // PERF_RECORD_MMAP
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L877
    // struct {
    //     struct perf_event_header header;
    //     u32 pid, tid;
    //     u64 addr;
    //     u64 len;
    //     u64 pgoff;
    //     char filename[];
    //     struct sample_id sample_id;
    // };
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        misc: u16,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let addr = deref_offset(&mut ptr);
        let len = deref_offset(&mut ptr);
        let page_offset = deref_offset(&mut ptr);
        let file = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(file.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8992
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        let executable = misc as u32 & b::PERF_RECORD_MISC_MMAP_DATA == 0;

        Self {
            record_id,
            executable,
            task,
            addr,
            len,
            file,
            page_offset,
        }
    }
}
