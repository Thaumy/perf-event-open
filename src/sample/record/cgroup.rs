use std::ffi::CString;

use super::RecordId;

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cgroup {
    pub record_id: Option<RecordId>,

    pub id: u64,
    pub path: CString,
}

impl Cgroup {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::ffi::CStr;
        use std::mem::align_of;

        use super::SampleType;
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1187
        // struct {
        //     struct perf_event_header header;
        //     u64 id;
        //     char path[];
        //     struct sample_id sample_id;
        // };

        let id = deref_offset(&mut ptr);
        let path = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(path.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8791
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        Self {
            record_id,
            id,
            path,
        }
    }
}

super::from!(Cgroup);

super::debug!(Cgroup {
    {record_id?},
    {id},
    {path},
});
