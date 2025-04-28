#[cfg(test)]
mod test;

use std::ffi::CStr;
use std::io::Result;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};

const TYPE_PATH: &str = "/sys/bus/event_source/devices/kprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/kprobe/format/retprobe";

/// Kernel probe event
#[derive(Clone, Debug)]
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

/// Kernel return probe event
#[derive(Clone, Debug)]
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
