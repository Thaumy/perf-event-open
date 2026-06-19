use super::EventConfig;
use crate::ffi::bindings as b;

/// A "raw" implementation-specific event.
///
/// # Examples
///
/// Raw event encodings are CPU-specific; consult your processor's manual
/// (e.g. the Intel SDM or AMD PPR) for the event and umask values.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::raw::Raw;
///
/// // Retired instructions on most Intel and AMD CPUs (event 0xc0, umask 0x00).
/// let event = Raw {
///     config: 0xc0,
///     config1: 0,
///     config2: 0,
///     config3: 0,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// std::hint::black_box((0..1_000_u64).sum::<u64>());
/// counter.disable().unwrap(); // Stop the counter.
///
/// let count = counter.stat().unwrap().count;
/// println!("raw event count: {}", count);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
