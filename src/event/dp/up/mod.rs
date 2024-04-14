#[cfg(test)]
mod test;

use std::ffi::CStr;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};

const TYPE_PATH: &str = "/sys/bus/event_source/devices/uprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/kprobe/format/retprobe";

#[derive(Clone, Debug)]
pub struct Uprobe {
    pub path: &'static CStr,
    pub offset: u64,
}

impl Uprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu, Error> {
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

    fn try_from(value: Uprobe) -> Result<Self, Self::Error> {
        value.try_into_dp()
    }
}

#[derive(Clone, Debug)]
pub struct Uretprobe {
    pub path: &'static CStr,
    pub offset: u64,
}

impl Uretprobe {
    pub fn try_into_dp(self) -> Result<DynamicPmu, Error> {
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

    fn try_from(value: Uretprobe) -> Result<Self, Self::Error> {
        value.try_into_dp()
    }
}
