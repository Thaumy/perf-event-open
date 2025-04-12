use super::EventConfig;
use crate::ffi::bindings as b;

/// A "raw" implementation-specific event.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Raw {
    /// Event config.
    pub config: u64,
    /// Event config1.
    pub config1: u64,
    /// Event config2.
    pub config2: u64,
    /// Event config3.
    ///
    /// Since `linux-6.3`: <https://github.com/torvalds/linux/commit/09519ec3b19e4144b5f6e269c54fbb9c294a9fcb>
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
