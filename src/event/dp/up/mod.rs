#[cfg(test)]
mod test;

use std::ffi::CStr;
use std::io::Result;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};
use crate::event::Event;

const TYPE_PATH: &str = "/sys/bus/event_source/devices/uprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/uprobe/format/retprobe";

/// User probe event
///
/// # Examples
///
/// Count calls to a function in the current executable. The probe offset
/// is the function's offset within the ELF file, which we derive from
/// `/proc/self/maps` so this works for both PIE and non-PIE binaries.
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::Uprobe;
///
/// // A function in the current executable to probe. `#[inline(never)]`
/// // and `black_box` keep it from being inlined or optimized away.
/// #[inline(never)]
/// fn probed(x: u64) -> u64 {
///     std::hint::black_box(x).wrapping_add(1)
/// }
///
/// // Each `/proc/self/maps` line is `start-end perms file_offset ...`, so
/// // the file offset of `probed` is `file_offset + (addr - start)` for the
/// // mapping that contains it.
/// let addr = probed as *const () as usize;
/// let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
/// let offset = maps
///     .lines()
///     .find_map(|line| {
///         let mut cols = line.split_whitespace();
///         let (start, end) = cols.next()?.split_once('-')?;
///         let start = usize::from_str_radix(start, 16).ok()?;
///         let end = usize::from_str_radix(end, 16).ok()?;
///         let file_offset = usize::from_str_radix(cols.nth(1)?, 16).ok()?;
///         (start..end)
///             .contains(&addr)
///             .then(|| (addr - start + file_offset) as u64)
///     })
///     .unwrap();
///
/// let event = Uprobe {
///     path: c"/proc/self/exe",
///     offset,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// let mut acc = 0;
/// for _ in 0..10 {
///     acc = probed(acc);
/// }
/// std::hint::black_box(acc);
/// counter.disable().unwrap(); // Stop the counter.
///
/// let calls = counter.stat().unwrap().count;
/// println!("{} calls to `probed`", calls);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Uprobe {
    /// Path to an executable or a library.
    pub path: &'static CStr,
    /// Where the probe is inserted.
    pub offset: u64,
}

impl Uprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu> {
        let ev = DynamicPmu {
            ty: get_type(TYPE_PATH)?,
            config: 0,
            config1: self.path.as_ptr() as _,
            config2: self.offset,
            config3: 0,
        };
        Ok(ev)
    }
}

impl TryFrom<Uprobe> for DynamicPmu {
    type Error = Error;

    fn try_from(value: Uprobe) -> Result<Self> {
        value.try_into_dp()
    }
}

impl TryFrom<Uprobe> for Event {
    type Error = Error;

    fn try_from(value: Uprobe) -> Result<Self> {
        value.try_into_dp()?.try_into()
    }
}

/// User return probe event
///
/// # Examples
///
/// Count returns from a function in the current executable. The probe
/// offset is the function's offset within the ELF file, which we derive
/// from `/proc/self/maps` so this works for both PIE and non-PIE binaries.
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::Uretprobe;
///
/// // A function in the current executable to probe. `#[inline(never)]`
/// // and `black_box` keep it from being inlined or optimized away.
/// #[inline(never)]
/// fn probed(x: u64) -> u64 {
///     std::hint::black_box(x).wrapping_add(1)
/// }
///
/// // Each `/proc/self/maps` line is `start-end perms file_offset ...`, so
/// // the file offset of `probed` is `file_offset + (addr - start)` for the
/// // mapping that contains it.
/// let addr = probed as *const () as usize;
/// let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
/// let offset = maps
///     .lines()
///     .find_map(|line| {
///         let mut cols = line.split_whitespace();
///         let (start, end) = cols.next()?.split_once('-')?;
///         let start = usize::from_str_radix(start, 16).ok()?;
///         let end = usize::from_str_radix(end, 16).ok()?;
///         let file_offset = usize::from_str_radix(cols.nth(1)?, 16).ok()?;
///         (start..end)
///             .contains(&addr)
///             .then(|| (addr - start + file_offset) as u64)
///     })
///     .unwrap();
///
/// let event = Uretprobe {
///     path: c"/proc/self/exe",
///     offset,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// let mut acc = 0;
/// for _ in 0..10 {
///     acc = probed(acc);
/// }
/// std::hint::black_box(acc);
/// counter.disable().unwrap(); // Stop the counter.
///
/// let returns = counter.stat().unwrap().count;
/// println!("{} returns from `probed`", returns);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Uretprobe {
    /// Path to an executable or a library.
    pub path: &'static CStr,
    /// Where the probe is inserted.
    pub offset: u64,
}

impl Uretprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu> {
        let ev = DynamicPmu {
            ty: get_type(TYPE_PATH)?,
            config: 1 << get_retprobe_bit(RETPROBE_PATH)?,
            config1: self.path.as_ptr() as _,
            config2: self.offset,
            config3: 0,
        };
        Ok(ev)
    }
}

impl TryFrom<Uretprobe> for DynamicPmu {
    type Error = Error;

    fn try_from(value: Uretprobe) -> Result<Self> {
        value.try_into_dp()
    }
}

impl TryFrom<Uretprobe> for Event {
    type Error = Error;

    fn try_from(value: Uretprobe) -> Result<Self> {
        value.try_into_dp()?.try_into()
    }
}
