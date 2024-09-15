use std::mem::MaybeUninit;

use crate::ffi::{bindings as b, deref_offset};
use crate::sample::record::debug;

/// Event statistics.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stat {
    /// Event count.
    pub count: u64,

    /// Event ID.
    pub id: Option<u64>,

    /// The enabled time of the counter.
    ///
    /// This can be used to calculate estimated totals if the PMU is overcommitted and multiplexing is happening,
    /// the raw count can be scaled by `raw / (time_running / time_enabled)` to estimate the totals.
    pub time_enabled: Option<u64>,

    /// The running time of the counter.
    ///
    /// Usually used together with [`time_enabled`][Self::time_enabled].
    pub time_running: Option<u64>,

    /// The number of lost records.
    ///
    /// If the sampler ring-buffer has no more space to hold the new records
    /// or the ring-buffer output is paused, the records are considered lost.
    ///
    /// Since `linux-6.0`: <https://github.com/torvalds/linux/commit/119a784c81270eb88e573174ed2209225d646656>
    pub lost_records: Option<u64>,

    /// Sibling event statistics.
    pub siblings: Vec<SiblingStat>,
}

impl Stat {
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L344
    // struct read_format {
    //     {
    //         u64 value;
    //         { u64 time_enabled; } && PERF_FORMAT_TOTAL_TIME_ENABLED
    //         { u64 time_running; } && PERF_FORMAT_TOTAL_TIME_RUNNING
    //         { u64 id;           } && PERF_FORMAT_ID
    //         { u64 lost;         } && PERF_FORMAT_LOST
    //     } && !PERF_FORMAT_GROUP
    //     {
    //         u64 nr;
    //         { u64 time_enabled; } && PERF_FORMAT_TOTAL_TIME_ENABLED
    //         { u64 time_running; } && PERF_FORMAT_TOTAL_TIME_RUNNING
    //         {
    //             u64 value;
    //             { u64 id;   } && PERF_FORMAT_ID
    //             { u64 lost; } && PERF_FORMAT_LOST
    //         } cntr[nr];
    //     } && PERF_FORMAT_GROUP
    // };
    pub(crate) unsafe fn from_ptr_offset(ptr: &mut *const u8, read_format: u64) -> Self {
        macro_rules! when {
            ($flag:ident, $ty:ty) => {
                (read_format & (b::$flag as u64) > 0).then(|| deref_offset::<$ty>(ptr))
            };
        }

        if read_format & b::PERF_FORMAT_GROUP as u64 == 0 {
            let count = deref_offset(ptr);
            let time_enabled = when!(PERF_FORMAT_TOTAL_TIME_ENABLED, u64);
            let time_running = when!(PERF_FORMAT_TOTAL_TIME_RUNNING, u64);
            let id = when!(PERF_FORMAT_ID, u64);
            #[cfg(feature = "linux-6.0")]
            let lost_records = when!(PERF_FORMAT_LOST, u64);
            #[cfg(not(feature = "linux-6.0"))]
            let lost_records = None;

            Self {
                count,
                id,
                time_enabled,
                time_running,
                lost_records,
                siblings: vec![],
            }
        } else {
            let nr: u64 = deref_offset(ptr);
            let time_enabled = when!(PERF_FORMAT_TOTAL_TIME_ENABLED, u64);
            let time_running = when!(PERF_FORMAT_TOTAL_TIME_RUNNING, u64);

            let count = deref_offset(ptr);
            let id = when!(PERF_FORMAT_ID, u64);
            #[cfg(feature = "linux-6.0")]
            let lost_records = when!(PERF_FORMAT_LOST, u64);
            #[cfg(not(feature = "linux-6.0"))]
            let lost_records = None;

            let siblings = (1..nr)
                .map(|_| {
                    let count = deref_offset(ptr);
                    let id = when!(PERF_FORMAT_ID, u64);
                    #[cfg(feature = "linux-6.0")]
                    let lost_records = when!(PERF_FORMAT_LOST, u64);
                    #[cfg(not(feature = "linux-6.0"))]
                    let lost_records = None;

                    SiblingStat {
                        count,
                        id,
                        lost_records,
                    }
                })
                .collect();

            Self {
                count,
                id,
                time_enabled,
                time_running,
                lost_records,
                siblings,
            }
        }
    }

    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, read_format: u64) -> Self {
        Self::from_ptr_offset(&mut ptr, read_format)
    }

    pub(crate) fn alloc_read_buf(
        base: &mut Vec<MaybeUninit<u8>>,
        group_size: usize,
        read_format: u64,
    ) {
        let mut size = size_of::<u64>();

        macro_rules! when {
            ($flag:ident, $size:expr) => {
                if read_format & b::$flag as u64 > 0 {
                    size += $size;
                }
            };
        }

        when!(PERF_FORMAT_TOTAL_TIME_ENABLED, size_of::<u64>());
        when!(PERF_FORMAT_TOTAL_TIME_RUNNING, size_of::<u64>());
        when!(PERF_FORMAT_GROUP, group_size * size_of::<u64>());
        when!(PERF_FORMAT_ID, group_size * size_of::<u64>());
        #[cfg(feature = "linux-6.0")]
        when!(PERF_FORMAT_LOST, group_size * size_of::<u64>());

        base.resize(size, MaybeUninit::uninit());
    }
}

debug!(Stat {
    {count},
    {id?},
    {time_enabled?},
    {time_running?},
    {lost_records?},
    {siblings},
});

/// Sibling event statistics.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SiblingStat {
    /// Event count.
    pub count: u64,

    /// Event ID.
    pub id: Option<u64>,

    /// The number of lost records.
    ///
    /// If the sampler ring-buffer has no more space to hold the new records
    /// or the ring-buffer output is paused, the records are considered lost.
    ///
    /// Since `linux-6.0`: <https://github.com/torvalds/linux/commit/119a784c81270eb88e573174ed2209225d646656>
    pub lost_records: Option<u64>,
}

debug!(SiblingStat {
    {count},
    {id?},
    {lost_records?},
});
