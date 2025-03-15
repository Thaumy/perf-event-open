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
/// Some records have been lost.
///
/// # Examples
///
/// ```rust
/// # #[cfg(not(feature = "linux-6.0"))]
/// # return;
/// #
/// # tokio_test::block_on(async {
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc, SampleOn, WakeUpOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
/// use perf_event_open::sample::record::Record;
///
/// let event = Software::TaskClock;
/// let target = (Proc::ALL, Cpu(0));
///
/// let mut opts = Opts::default();
/// opts.stat_format.lost_records = true;
/// opts.wake_up.on = WakeUpOn::Samples(1);
/// opts.sample_on = SampleOn::Count(1_000_000); // 1ms
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
/// // Pause the ring-buffer output, discarded samples are considered lost.
/// sampler.pause().unwrap();
/// thread::sleep(Duration::from_millis(10));
/// sampler.resume().unwrap();
///
/// let mut iter = sampler.iter().into_async().unwrap();
/// while let Some((_, r)) = iter.next().await {
///     if let Record::LostRecords(l) = r {
///         println!("{:-?}", l);
///         # assert!(l.lost_records > 0);
///         break;
///     }
/// }
/// # });
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LostRecords {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Event ID.
    pub id: u64,
    /// The number of lost records.
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

super::debug!(LostRecords {
    {record_id?},
    {id},
    {lost_records},
});

/// Since `linux-4.2`: <https://github.com/torvalds/linux/commit/f38b0dbb491a6987e198aa6b428db8692a6480f8>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LostSamples {
    pub record_id: Option<RecordId>,

    pub lost_samples: u64,
}

impl LostSamples {
    #[cfg(feature = "linux-4.2")]
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

super::debug!(LostSamples {
    {record_id?},
    {lost_samples},
});
