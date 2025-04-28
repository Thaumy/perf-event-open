#[cfg(test)]
mod test;

use std::ffi::CStr;
use std::io::Result;

use super::{get_retprobe_bit, get_type, DynamicPmu, Error};

const TYPE_PATH: &str = "/sys/bus/event_source/devices/uprobe/type";
const RETPROBE_PATH: &str = "/sys/bus/event_source/devices/kprobe/format/retprobe";

/// User probe event
#[derive(Clone, Debug)]
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

/// User return probe event
#[derive(Clone, Debug)]
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
