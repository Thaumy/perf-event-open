use super::EventConfig;
use crate::ffi::bindings as b;

/// Software events provided by the kernel.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Software {
    /// This reports the CPU clock, a high-resolution per-CPU timer.
    CpuClock,
    /// This reports a clock count specific to the task that is running (in nanoseconds).
    TaskClock,

    /// The number of page faults.
    PageFault,
    /// The number of minor page faults. These did not require disk I/O to handle.
    MinorPageFault,
    /// The number of major page faults. These required disk I/O to handle.
    MajorPageFault,

    /// The number of emulation faults.
    ///
    /// The kernel sometimes traps on unimplemented instructions and emulates them for
    /// user space. This can negatively impact performance.
    EmuFault,
    /// The number of alignment faults.
    ///
    /// These happen when unaligned memory accesses happen; the kernel can handle these but
    /// it reduces performance. This happens only on some architectures (never on x86).
    AlignFault,

    /// This number of context switches.
    CtxSwitch,
    /// This counts context switches to a task in a different cgroup.
    ///
    /// In other words, if the next task is in the same cgroup, it won't count the switch.
    ///
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/d0d1dd628527c77db2391ce0293c1ed344b2365f>
    CgroupSwitch,

    /// A placeholder event that counts nothing.
    ///
    /// Informational sample record types such as [`Mmap`][crate::sample::record::mmap::Mmap]
    /// or [`Comm`][crate::sample::record::comm::Comm] must be associated with an active event.
    /// This dummy event allows gathering such records without requiring a counting event.
    Dummy,
    /// This is used to generate raw sample data from BPF.
    ///
    /// BPF programs can write to this event using `bpf_perf_event_output` helper.
    ///
    /// Since `linux-4.4`: <https://github.com/torvalds/linux/commit/a43eec304259a6c637f4014a6d4767159b6a3aa3>
    BpfOutput,

    /// The number of times the process has migrated to a new CPU.
    CpuMigration,
}

super::try_from!(Software, value, {
    let config = match value {
        Software::CpuClock => b::PERF_COUNT_SW_CPU_CLOCK,
        Software::TaskClock => b::PERF_COUNT_SW_TASK_CLOCK,

        Software::PageFault => b::PERF_COUNT_SW_PAGE_FAULTS,
        Software::MinorPageFault => b::PERF_COUNT_SW_PAGE_FAULTS_MIN,
        Software::MajorPageFault => b::PERF_COUNT_SW_PAGE_FAULTS_MAJ,

        Software::EmuFault => b::PERF_COUNT_SW_EMULATION_FAULTS,
        Software::AlignFault => b::PERF_COUNT_SW_ALIGNMENT_FAULTS,

        Software::CtxSwitch => b::PERF_COUNT_SW_CONTEXT_SWITCHES,
        #[cfg(feature = "linux-5.13")]
        Software::CgroupSwitch => b::PERF_COUNT_SW_CGROUP_SWITCHES,

        Software::Dummy => b::PERF_COUNT_SW_DUMMY,
        #[cfg(feature = "linux-4.4")]
        Software::BpfOutput => b::PERF_COUNT_SW_BPF_OUTPUT,
        Software::CpuMigration => b::PERF_COUNT_SW_CPU_MIGRATIONS,

        #[cfg(not(feature = "linux-5.13"))]
        _ => crate::config::unsupported!(),
    };
    let event_config = EventConfig {
        ty: b::PERF_TYPE_SOFTWARE,
        config: config as _,
        config1: 0,
        config2: 0,
        config3: 0,
        bp_type: 0,
    };
    Ok(Self(event_config))
});
