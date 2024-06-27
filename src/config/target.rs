use std::fs::File;
use std::os::fd::AsRawFd;

use crate::ffi::bindings as b;

/// Monitor all processes (if [`Proc`] is not set) or all CPUs (if [`Cpu`] is not set).
#[derive(Clone, Copy, Debug)]
pub struct All;

/// Which CPU to monitor.
#[derive(Clone, Copy, Debug)]
pub struct Cpu(pub u32);

impl Cpu {
    /// Monitor all CPUs.
    ///
    /// This is an alias for [`All`].
    pub const ALL: All = All;
}

/// Which process (thread) to monitor.
///
/// Construct with pid or tid.
///
/// `Proc(0)` indicates the current process.
#[derive(Clone, Copy, Debug)]
pub struct Proc(pub u32);

impl Proc {
    /// Monitor all processes.
    ///
    /// This is an alias for [`All`].
    pub const ALL: All = All;

    /// Monitor current process.
    pub const CURRENT: Proc = Proc(0);
}

/// Which cgroup to monitor.
///
/// For instance, if the cgroup to monitor is called test, then a file descriptor opened on
/// `/dev/cgroup/test` (assuming cgroupfs is mounted on `/dev/cgroup`) should be passed.
///
/// cgroup monitoring is available only for system-wide events and may therefore require
/// extra permissions.
#[derive(Clone, Copy, Debug)]
pub struct Cgroup<'a>(pub &'a File);

/// Event target, the process (or cgroup) and CPU to monitor.
///
/// To create an event target, combine these types in a tuple: [`Proc`] (or [`Cgroup`]), [`Cpu`] and [`All`].
///
/// For example, we want to monitor process with pid 12345 on all CPUs: `(Pid(12345), Cpu::ALL)`.
/// The order of types in the tuples is not senstive because we impl `Into<Target>` for these
/// swapped tuples, e.g. `(Cpu::ALL, Pid(12345))` has the same semantics as the example above.
///
/// This design limits what we can monitor at compile time. For example, the kernel not support
/// monitoring any process on all CPUs, or a cgroup on all CPUs.
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
