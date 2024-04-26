use super::{RecordId, SampleType};
use crate::ffi::deref_offset;

// PERF_RECORD_LOST counts all lost records:
// Count lost when paused:
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L178
// Count lost when no space:
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L203
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L263
// Generate PERF_RECORD_LOST:
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L189
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L247
// The same applies to `perf_read`:
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L5764
// https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7373
#[derive(Clone)]
pub struct LostRecords {
    pub record_id: Option<RecordId>,

    pub id: u64,
    pub lost_records: u64,
}

impl LostRecords {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L891
        // struct {
        //     struct perf_event_header header;
        //     u64 id;
        //     u64 lost;
        //     struct sample_id sample_id;
        // };

        let id = deref_offset(&mut ptr);
        let lost_records = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            id,
            lost_records,
        }
    }
}

super::from!(LostRecords);

#[derive(Clone)]
pub struct LostSamples {
    pub record_id: Option<RecordId>,

    pub lost_samples: u64,
}

impl LostSamples {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1105
        // struct {
        //     struct perf_event_header header;
        //     u64 lost;
        //     struct sample_id sample_id;
        // };

        let lost_samples = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            lost_samples,
        }
    }
}

super::from!(LostSamples);
