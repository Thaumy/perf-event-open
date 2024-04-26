use crate::ffi::{bindings as b, deref_offset};
use crate::sample::record::debug;

#[derive(Clone)]
pub struct Stat {
    pub count: u64,
    pub id: Option<u64>,
    pub time_enabled: Option<u64>,
    pub time_running: Option<u64>,
    pub lost_records: Option<u64>,
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
            let lost_records = when!(PERF_FORMAT_LOST, u64);

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
            let lost_records = when!(PERF_FORMAT_LOST, u64);

            let siblings = (1..nr)
                .map(|_| {
                    let count = deref_offset(ptr);
                    let id = when!(PERF_FORMAT_ID, u64);
                    let lost_records = when!(PERF_FORMAT_LOST, u64);

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
}

debug!(Stat {
    {count},
    {id?},
    {time_enabled?},
    {time_running?},
    {lost_records?},
    {siblings},
});

#[derive(Clone)]
pub struct SiblingStat {
    pub count: u64,
    pub id: Option<u64>,
    pub lost_records: Option<u64>,
}

debug!(SiblingStat {
    {count},
    {id?},
    {lost_records?},
});
