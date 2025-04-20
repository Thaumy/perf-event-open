use super::{Uprobe, Uretprobe};
use crate::event::dp::DynamicPmu;

#[test]
fn test_from_uprobe() {
    let ev = Uprobe {
        path: c"",
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}

#[test]
fn test_from_uretprobe() {
    let ev = Uretprobe {
        path: c"",
        offset: 0,
    };
    DynamicPmu::try_from(ev).unwrap();
}
