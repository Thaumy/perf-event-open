use super::EventConfig;
use crate::ffi::bindings as b;

#[derive(Clone, Debug)]
pub struct Tracepoint {
    pub id: u64,
}

super::try_from!(Tracepoint, value, {
    let event_config = EventConfig {
        ty: b::PERF_TYPE_TRACEPOINT,
        config: value.id,
        config1: 0,
        config2: 0,
        config3: 0,
        bp_type: 0,
    };
    Ok(Self(event_config))
});
