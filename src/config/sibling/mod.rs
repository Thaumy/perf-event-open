use std::io::Result;

use super::{
    ExtraRecord, Inherit, OnExecve, Priv, RecordIdFormat, SampleFormat, SampleOn, SampleSkid,
    SigData, WakeUp,
};
use crate::ffi::bindings as b;

pub(crate) mod attr;

#[derive(Clone, Debug, Default)]
pub struct Opts {
    pub exclude: Priv,
    pub inherit: Option<Inherit>,
    pub on_execve: Option<OnExecve>,
    pub stat_format: StatFormat,

    pub enable: bool,
    pub sample_on: SampleOn,
    pub sample_skid: SampleSkid,
    pub sample_format: SampleFormat,
    pub extra_record: ExtraRecord,
    pub record_id_all: bool,
    pub record_id_format: RecordIdFormat,
    pub wake_up: WakeUp,
    // https://github.com/torvalds/linux/commit/ab43762ef010967e4ccd53627f70a2eecbeafefb
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L2152
    /// Since `linux-5.4`: <https://github.com/torvalds/linux/commit/ab43762ef010967e4ccd53627f70a2eecbeafefb>
    pub aux_output: bool,
    pub on_sample: OnSample,
}

/// Controls the format of [`Stat`][crate::count::Stat].
#[derive(Clone, Debug, Default)]
pub struct StatFormat {
    /// Contains the event ID ([`Stat::id`][crate::count::Stat::id]
    /// and [`SiblingStat::id`][crate::count::SiblingStat::id]).
    pub id: bool,

    /// Contains the [enabled time][crate::count::Stat::time_enabled] of the counter.
    pub time_enabled: bool,

    /// Contains the [running time][crate::count::Stat::time_running] of the counter.
    pub time_running: bool,

    /// Contains the number of lost records ([`Stat::lost_records`][crate::count::Stat::lost_records].
    /// and [`SiblingStat::lost_records`][crate::count::SiblingStat::lost_records]).
    ///
    /// Since `linux-6.0`: <https://github.com/torvalds/linux/commit/119a784c81270eb88e573174ed2209225d646656>
    pub lost_records: bool,
}

impl StatFormat {
    pub(crate) fn as_read_format(&self) -> Result<u64> {
        let mut val = 0;
        macro_rules! when {
            ($field:ident, $flag:ident) => {
                if self.$field {
                    val |= b::$flag;
                }
            };
        }
        when!(id, PERF_FORMAT_ID);
        when!(time_enabled, PERF_FORMAT_TOTAL_TIME_ENABLED);
        when!(time_running, PERF_FORMAT_TOTAL_TIME_RUNNING);
        #[cfg(feature = "linux-6.0")]
        when!(lost_records, PERF_FORMAT_LOST);
        #[cfg(not(feature = "linux-6.0"))]
        crate::config::unsupported!(self.lost_records);
        Ok(val as _)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OnSample {
    /// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
    pub aux: Option<AuxTracer>,

    // Must be used together with `remove_on_exec`:
    // https://github.com/torvalds/linux/blob/2408a807bfc3f738850ef5ad5e3fd59d66168996/kernel/events/core.c#L12582
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/97ba62b278674293762c3d91f724f1bb922f04e0>
    pub sigtrap: Option<SigData>,
}

/// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
#[derive(Clone, Debug)]
pub enum AuxTracer {
    Pause,
    Resume,
}
