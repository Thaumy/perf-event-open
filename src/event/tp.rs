use super::EventConfig;
use crate::ffi::bindings as b;

/// Tracepoint event provided by the kernel tracepoint infrastructure.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tracepoint {
    /// Tracepoint ID from under debugfs `tracing/events/*/*/id` if ftrace is enabled in the kernel.
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
