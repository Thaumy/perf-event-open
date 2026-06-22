#[cfg(test)]
mod test;

use std::ffi::CStr;
use std::io::Result;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};
use crate::event::Event;

const TYPE_PATH: &str = "/sys/bus/event_source/devices/kprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/kprobe/format/retprobe";

/// Kernel probe event
///
/// # Examples
///
/// Count calls to the kernel function `do_sys_openat2` (which handles the
/// `openat` syscall) by opening files.
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::Kprobe;
///
/// // Probe the entry of `do_sys_openat2`.
/// let event = Kprobe::Symbol {
///     name: c"do_sys_openat2",
///     offset: 0,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// for _ in 0..10 {
///     let _ = std::fs::File::open("/proc/self/status");
/// }
/// counter.disable().unwrap(); // Stop the counter.
///
/// let hits = counter.stat().unwrap().count;
/// println!("{} calls to do_sys_openat2", hits);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kprobe {
    /// Symbol + offset where the probe is inserted.
    Symbol { name: &'static CStr, offset: u64 },
    /// Address where the probe is inserted.
    Addr(u64),
}

impl Kprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu> {
        let ty = get_type(TYPE_PATH)?;
        let ev = match self {
            Kprobe::Symbol { name, offset } => DynamicPmu {
                ty,
                config: 0,
                config1: name.as_ptr() as _,
                config2: offset,
                config3: 0,
            },
            Kprobe::Addr(addr) => DynamicPmu {
                ty,
                config: 0,
                config1: 0,
                config2: addr,
                config3: 0,
            },
        };
        Ok(ev)
    }
}

impl TryFrom<Kprobe> for DynamicPmu {
    type Error = Error;

    fn try_from(value: Kprobe) -> Result<Self> {
        value.try_into_dp()
    }
}

impl TryFrom<Kprobe> for Event {
    type Error = Error;

    fn try_from(value: Kprobe) -> Result<Self> {
        value.try_into_dp()?.try_into()
    }
}

/// Kernel return probe event
///
/// # Examples
///
/// Count returns from the kernel function `do_sys_openat2` by opening files.
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::Kretprobe;
///
/// // Probe the return of `do_sys_openat2`.
/// let event = Kretprobe::Symbol {
///     name: c"do_sys_openat2",
///     offset: 0,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// for _ in 0..10 {
///     let _ = std::fs::File::open("/proc/self/status");
/// }
/// counter.disable().unwrap(); // Stop the counter.
///
/// let hits = counter.stat().unwrap().count;
/// println!("{} returns from do_sys_openat2", hits);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kretprobe {
    /// Symbol + offset where the probe is inserted.
    Symbol { name: &'static CStr, offset: u64 },
    /// Address where the probe is inserted.
    Addr(u64),
}

impl Kretprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu> {
        let ty = get_type(TYPE_PATH)?;
        let retprobe_bit = get_retprobe_bit(RETPROBE_PATH)?;
        let ev = match self {
            Kretprobe::Symbol { name, offset } => DynamicPmu {
                ty,
                config: 1 << retprobe_bit,
                config1: name.as_ptr() as _,
                config2: offset,
                config3: 0,
            },
            Kretprobe::Addr(addr) => DynamicPmu {
                ty,
                config: 1 << retprobe_bit,
                config1: 0,
                config2: addr,
                config3: 0,
            },
        };
        Ok(ev)
    }
}

impl TryFrom<Kretprobe> for DynamicPmu {
    type Error = Error;

    fn try_from(value: Kretprobe) -> Result<Self> {
        value.try_into_dp()
    }
}

impl TryFrom<Kretprobe> for Event {
    type Error = Error;

    fn try_from(value: Kretprobe) -> Result<Self> {
        value.try_into_dp()?.try_into()
    }
}
