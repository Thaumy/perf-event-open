mod kp;
mod up;

use std::fs::File;
use std::io::{Error, Read, Result, Seek, SeekFrom};
use std::path::Path;

pub use kp::*;
use thiserror::Error;
pub use up::*;

use super::EventConfig;

/// Dynamic PMU event
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DynamicPmu {
    /// The type value to use can be found in the sysfs filesystem: there is a subdirectory per
    /// PMU instance under `/sys/bus/event_source/devices`. In each subdirectory there is a
    /// type file whose content is an integer that can be used in the this field.
    ///
    /// For instance, `/sys/bus/event_source/devices/cpu/type` contains the value for
    /// the core CPU PMU, which is usually 4.
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
