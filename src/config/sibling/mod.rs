use std::io::Result;

use super::{
    ExtraRecord, Inherit, OnExecve, Priv, RecordIdFormat, SampleFormat, SampleOn, SampleSkid,
    SigData, WakeUp,
};
use crate::ffi::bindings as b;

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
    pub aux_output: bool,
    pub on_sample: OnSample,
}

#[derive(Clone, Debug, Default)]
pub struct StatFormat {
    pub id: bool,
    pub time_enabled: bool,
    pub time_running: bool,
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
        when!(lost_records, PERF_FORMAT_LOST);
        Ok(val as _)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OnSample {
    pub aux: Option<AuxTracer>,

    // Must be used together with `remove_on_exec`:
    // https://github.com/torvalds/linux/blob/2408a807bfc3f738850ef5ad5e3fd59d66168996/kernel/events/core.c#L12582
    pub sigtrap: Option<SigData>,
}

#[derive(Clone, Debug)]
pub enum AuxTracer {
    Pause,
    Resume,
}
