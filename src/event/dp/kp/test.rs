use super::{Kprobe, Kretprobe};
use crate::event::dp::DynamicPmu;

#[test]
fn test_from_kprobe_func() {
    let ev = Kprobe::Symbol {
        name: c"",
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
        name: c"",
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_kretprobe_addr() {
    let ev = Kretprobe::Addr(0);
    DynamicPmu::try_from(ev).unwrap();
}
