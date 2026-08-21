mod kp;
mod up;

use std::fs::File;
use std::io::{Error, Read, Result, Seek, SeekFrom};
use std::path::Path;

pub use kp::*;
pub use up::*;

use super::EventConfig;

/// Dynamic PMU event
///
/// # Examples
///
/// Access a PMU exposed under `/sys/bus/event_source/devices` directly. Here we use
/// the always-available `software` PMU to count page faults:
///
/// ```rust
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::DynamicPmu;
///
/// let type_path = "/sys/bus/event_source/devices/software/type";
/// let ty = std::fs::read_to_string(type_path)
///     .unwrap()
///     .trim()
///     .parse()
///     .unwrap();
///
/// let event = DynamicPmu {
///     ty,
///     config: 2, // PERF_COUNT_SW_PAGE_FAULTS
///     config1: 0,
///     config2: 0,
///     config3: 0,
/// };
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let counter = Counter::new(event, target, Opts::default()).unwrap();
///
/// counter.enable().unwrap(); // Start the counter.
/// // Touch every page of a fresh allocation to trigger page faults.
/// let mut buf = vec![0u8; 4096 * 1024];
/// for page in buf.chunks_mut(4096) {
///     page[0] = 1;
/// }
/// std::hint::black_box(&buf);
/// counter.disable().unwrap(); // Stop the counter.
///
/// let faults = counter.stat().unwrap().count;
/// println!("{} page faults", faults);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DynamicPmu {
    /// The type value to use can be found in the sysfs filesystem.
    ///
    /// For example, `/sys/bus/event_source/devices/cpu/type` contains the value
    /// for the core CPU PMU, which is usually 4.
    pub ty: u32,
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

fn get_type<P>(path: P) -> Result<u32>
where
    P: AsRef<Path>,
{
    let mut file = File::open(path)?;

    let mut acc = Vec::with_capacity(1);
    let mut buf = [0];
    while file.read(&mut buf)? > 0 {
        if buf[0] == b'\n' {
            break;
        }
        acc.extend(buf);
    }
    let bit = unsafe { std::str::from_utf8_unchecked(&acc) };

    bit.parse::<u32>().map_err(Error::other)
}

// bpf_get_retprobe_bit:
// https://github.com/torvalds/linux/blob/v6.13/samples/bpf/task_fd_query_user.c#L69
fn get_retprobe_bit<P>(path: P) -> Result<u8>
where
    P: AsRef<Path>,
{
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start("config:".len() as _))?;

    let mut acc = Vec::with_capacity(1);
    let mut buf = [0];
    while file.read(&mut buf)? > 0 {
        if buf[0] == b'\n' {
            break;
        }
        acc.extend(buf);
    }
    let bit = unsafe { std::str::from_utf8_unchecked(&acc) };

    bit.parse::<u8>().map_err(Error::other)
}

super::try_from!(DynamicPmu, value, {
    let event_cfg = EventConfig {
        ty: value.ty,
        config: value.config,
        config1: value.config1,
        config2: value.config2,
        config3: value.config3,
        bp_type: 0,
    };
    Ok(Self(event_cfg))
});
