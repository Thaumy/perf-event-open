use std::ffi::CStr;

use super::{Uprobe, Uretprobe};
use crate::event::dp::DynamicPmu;

#[test]
fn test_from_uprobe() {
    let ev = Uprobe {
        path: CStr::from_bytes_with_nul(b"\0").unwrap(),
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_uretprobe() {
    let ev = Uretprobe {
        path: CStr::from_bytes_with_nul(b"\0").unwrap(),
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}
