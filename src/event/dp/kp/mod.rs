#[cfg(test)]
mod test;

use std::ffi::CStr;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};

const TYPE_PATH: &str = "/sys/bus/event_source/devices/kprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/kprobe/format/retprobe";

#[derive(Clone, Debug)]
pub enum Kprobe {
    Symbol { name: &'static CStr, offset: u64 },
    Addr(u64),
}

impl Kprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu, Error> {
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

    fn try_from(value: Kprobe) -> Result<Self, Self::Error> {
        value.try_into_dp()
    }
}

#[derive(Clone, Debug)]
pub enum Kretprobe {
    Symbol { name: &'static CStr, offset: u64 },
    Addr(u64),
}

impl Kretprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu, Error> {
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

    fn try_from(value: Kretprobe) -> Result<Self, Self::Error> {
        value.try_into_dp()
    }
}
