use super::{RecordId, SampleType};
use crate::ffi::deref_offset;

#[derive(Clone)]
pub struct Throttle {
    pub record_id: Option<RecordId>,

    pub time: u64,
    pub id: u64,
    pub stream_id: u64,
}

impl Throttle {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L923
        // struct {
        //     struct perf_event_header header;
        //     u64 time;
        //     u64 id;
        //     u64 stream_id;
        //     struct sample_id sample_id;
        // };

        let time = deref_offset(&mut ptr);
        let id = deref_offset(&mut ptr);
        let stream_id = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            time,
            id,
            stream_id,
        }
    }
}
