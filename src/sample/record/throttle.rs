use super::{RecordId, SampleType};
use crate::ffi::deref_offset;

/// Sampling has been throttled.
///
/// This record indicates that the maximum sampling rate has been reached,
/// and kernel will correct the sampling rate to avoid exceeding the limit.
///
/// See also [`SampleOn`][crate::config::SampleOn].
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Throttle {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Timestamp.
    pub time: u64,
    /// Event ID.
    pub id: u64,
    /// Event stream ID.
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

super::from!(Throttle);

super::debug!(Throttle {
    {record_id?},
    {time},
    {id},
    {stream_id},
});

/// Sampling throttle has been lifted.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unthrottle {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Timestamp.
    pub time: u64,
    /// Event ID.
    pub id: u64,
    /// Event stream ID.
    pub stream_id: u64,
}

impl Unthrottle {
    pub(crate) unsafe fn from_ptr(ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9332
        let layout = Throttle::from_ptr(ptr, sample_id_all);

        Self {
            record_id: layout.record_id,
            time: layout.time,
            id: layout.id,
            stream_id: layout.stream_id,
        }
    }
}

super::from!(Unthrottle);

super::debug!(Unthrottle {
    {record_id?},
    {time},
    {id},
    {stream_id},
});
