use std::ffi::CStr;

use super::{Kprobe, Kretprobe};
use crate::event::dp::DynamicPmu;

#[test]
fn test_from_kprobe_func() {
    let ev = Kprobe::Symbol {
        name: CStr::from_bytes_with_nul(b"\0").unwrap(),
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_kprobe_addr() {
    let ev = Kprobe::Addr(0);
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_kretprobe_func() {
    let ev = Kretprobe::Symbol {
        name: CStr::from_bytes_with_nul(b"\0").unwrap(),
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_kretprobe_addr() {
    let ev = Kretprobe::Addr(0);
    DynamicPmu::try_from(ev).unwrap();
}
