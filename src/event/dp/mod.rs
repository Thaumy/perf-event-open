mod kp;
mod up;

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::num::ParseIntError;
use std::path::Path;

pub use kp::*;
use thiserror::Error;
pub use up::*;

use super::EventConfig;

#[derive(Clone, Debug)]
pub struct DynamicPmu {
    pub ty: u32,
    pub config: u64,
    pub config1: u64,
    pub config2: u64,
    /// Since `linux-6.3`: <https://github.com/torvalds/linux/commit/09519ec3b19e4144b5f6e269c54fbb9c294a9fcb>
    pub config3: u64,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to read probe type: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse probe type: {0}")]
    Parse(#[from] ParseIntError),
}

fn get_type<P>(path: P) -> Result<u32, Error>
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

    let bit = bit.parse::<u32>()?;
    Ok(bit)
}

// bpf_get_retprobe_bit:
// https://github.com/torvalds/linux/blob/v6.13/samples/bpf/task_fd_query_user.c#L69
fn get_retprobe_bit<P>(path: P) -> Result<u8, Error>
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

    let bit = bit.parse::<u8>()?;
    Ok(bit)
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
