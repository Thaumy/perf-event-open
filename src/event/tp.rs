use super::EventConfig;
use crate::ffi::bindings as b;

/// Tracepoint event provided by the kernel tracepoint infrastructure.
///
/// # Examples
///
/// Count `openat` syscalls via the `syscalls:sys_enter_openat` tracepoint.
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use std::fs::{File, read_to_string};
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::tp::Tracepoint;
///
/// // Tracepoint IDs live under the tracefs, usually mounted at
/// // `/sys/kernel/tracing` (or `/sys/kernel/debug/tracing`).
/// let id = read_to_string("/sys/kernel/tracing/events/syscalls/sys_enter_openat/id").unwrap();
/// let event = Tracepoint {
///     id: id.trim().parse().unwrap(),
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// for _ in 0..10 {
///     let _ = File::open("/proc/self/status");
/// }
/// counter.disable().unwrap(); // Stop the counter.
///
/// let openats = counter.stat().unwrap().count;
/// println!("{} openat syscalls", openats);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tracepoint {
    /// Tracepoint ID from `events/*/*/id` under tracefs or
    /// `tracing/events/*/*/id` under debugfs if ftrace is
    /// enabled in the kernel.
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
