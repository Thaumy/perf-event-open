use std::io::Result;

use super::{
    ExtraRecord, Inherit, OnExecve, Priv, RecordIdFormat, SampleFormat, SampleOn, SampleSkid,
    SigData, WakeUp,
};
use crate::ffi::bindings as b;

pub(crate) mod attr;

// We skipped some options for sibling event to make sure the attr is valid.
// * All events in a group should have the same clock:
// https://github.com/torvalds/linux/blob/7ff71e6d923969d933e1ba7e0db857782d36cd19/kernel/events/core.c#L12962
// * Only a group leader can be exclusive or pinned:
// https://github.com/torvalds/linux/blob/7ff71e6d923969d933e1ba7e0db857782d36cd19/kernel/events/core.c#L12982
/// Sibling event options.
#[derive(Clone, Debug, Default)]
pub struct Opts {
    /// Exclude events with privilege levels.
    ///
    /// For example, if we set [`Priv::user`] to `true` here,
    /// events that happen in user space will not be counted.
    pub exclude: Priv,

    /// Controls the inherit behavior.
    pub inherit: Option<Inherit>,

    /// Counter behavior when calling [`execve`](https://man7.org/linux/man-pages/man2/execve.2.html).
    pub on_execve: Option<OnExecve>,

    /// Controls the format of [`Stat`][crate::count::Stat].
    pub stat_format: StatFormat,

    /// Enable counter immediately after the counter is created.
    pub enable: bool,

    /// Controls when to generate a [sample record][crate::sample::record::sample::Sample].
    pub sample_on: SampleOn,

    /// Controls the amount of sample skid.
    pub sample_skid: SampleSkid,

    /// Controls the format of [sample record][crate::sample::record::sample::Sample].
    pub sample_format: SampleFormat,

    /// Generate extra record types.
    pub extra_record: ExtraRecord,

    /// Contains [`RecordId`][crate::sample::record::RecordId] in all non-sample [record][crate::sample::record] types.
    pub record_id_all: bool,

    /// Controls the format of [`RecordId`][crate::sample::record::RecordId].
    pub record_id_format: RecordIdFormat,

    /// Wake up options for asynchronous iterators.
    pub wake_up: WakeUp,

    // https://github.com/torvalds/linux/commit/ab43762ef010967e4ccd53627f70a2eecbeafefb
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L2152
    /// Enable sibling event to generate data for leader AUX event.
    ///
    /// In some cases, ordinary (non-AUX) events can generate data for AUX events.
    /// For example, PEBS events can come out as records in the Intel PT stream
    /// instead of their usual DS records, if configured to do so.
    ///
    /// This requires the group leader to be an AUX event.
    ///
    /// Since `linux-5.4`: <https://github.com/torvalds/linux/commit/ab43762ef010967e4ccd53627f70a2eecbeafefb>
    pub aux_output: bool,

    /// The action to perform when generating the [sample record][crate::sample::record::sample::Sample].
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

/// The action to perform when generating the [sample record][crate::sample::record::sample::Sample].
#[derive(Clone, Debug, Default)]
pub struct OnSample {
    /// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
    pub aux: Option<AuxTracer>,

    // Must be used together with `remove_on_exec`:
    // https://github.com/torvalds/linux/blob/2408a807bfc3f738850ef5ad5e3fd59d66168996/kernel/events/core.c#L12582
    /// Enables synchronous signal delivery of `SIGTRAP` to the target
    /// process on event overflow.
    ///
    /// Same as [`Opts::sigtrap_on_sample`][super::Opts::sigtrap_on_sample],
    /// but for sibling events.
    ///
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/97ba62b278674293762c3d91f724f1bb922f04e0>
    pub sigtrap: Option<SigData>,
}

/// AUX tracer action.
///
/// Since `linux-6.13`: <https://github.com/torvalds/linux/commit/18d92bb57c39504d9da11c6ef604f58eb1d5a117>
#[derive(Clone, Debug)]
pub enum AuxTracer {
    /// Pause [AUX tracer][crate::sample::auxiliary::AuxTracer].
    Pause,
    /// Resume [AUX tracer][crate::sample::auxiliary::AuxTracer].
    Resume,
}
