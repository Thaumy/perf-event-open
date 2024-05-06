use std::fs::File;
use std::os::fd::AsRawFd;

use crate::ffi::bindings as b;

#[derive(Clone, Copy, Debug)]
pub struct All;

#[derive(Clone, Copy, Debug)]
pub struct Cpu(pub u32);

impl Cpu {
    pub const ALL: All = All;
}

#[derive(Clone, Copy, Debug)]
pub struct Proc(pub u32);

impl Proc {
    pub const ALL: All = All;
    pub const CURRENT: Proc = Proc(0);
}

#[derive(Clone, Copy, Debug)]
pub struct Cgroup<'a>(pub &'a File);

#[derive(Clone)]
pub struct Target {
    pub(crate) pid: i32,
    pub(crate) cpu: i32,
    pub(crate) flags: u64,
}

macro_rules! into_target {
    ($ty: ty, $destruct: tt, $pid: expr, $cpu: expr, $flags: expr) => {
        impl From<$ty> for Target {
            fn from($destruct: $ty) -> Self {
                Target {
                    pid: $pid as _,
                    cpu: $cpu as _,
                    flags: $flags as _,
                }
            }
        }
    };
}

into_target!((Proc, Cpu), (Proc(pid), Cpu(cpu)), pid, cpu, 0);
into_target!((Cpu, Proc), (Cpu(cpu), Proc(pid)), pid, cpu, 0);

into_target!((Proc, All), (Proc(pid), _), pid, -1, 0);
into_target!((All, Proc), (_, Proc(pid)), pid, -1, 0);

into_target!((Cpu, All), (Cpu(cpu), _), -1, cpu, 0);
into_target!((All, Cpu), (_, Cpu(cpu)), -1, cpu, 0);

into_target!(
    (Cgroup<'_>, Cpu),
    (Cgroup(file), Cpu(cpu)),
    file.as_raw_fd(),
    cpu,
    b::PERF_FLAG_PID_CGROUP
);
into_target!(
    (Cpu, Cgroup<'_>),
    (Cpu(cpu), Cgroup(file)),
    file.as_raw_fd(),
    cpu,
    b::PERF_FLAG_PID_CGROUP
);

// For why `(CgroupFd, Any)` is invalid:
// https://github.com/torvalds/linux/blob/4dc1d1bec89864d8076e5ab314f86f46442bfb02/kernel/events/core.c#L12835
