use super::EventConfig;
use crate::ffi::bindings as b;

#[derive(Clone, Debug)]
pub struct Raw {
    pub config: u64,
    pub config1: u64,
    pub config2: u64,
    pub config3: u64,
}

super::try_from!(Raw, value, {
    let event_config = EventConfig {
        ty: b::PERF_TYPE_RAW,
        config: value.config,
        config1: value.config1,
        config2: value.config2,
        config3: value.config3,
        bp_type: 0,
    };
    Ok(Self(event_config))
});
