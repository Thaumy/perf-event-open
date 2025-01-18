use super::EventConfig;
use crate::ffi::bindings as b;

#[derive(Clone, Debug)]
pub enum Software {
    CpuClock,
    TaskClock,

    PageFault,
    MinorPageFault,
    MajorPageFault,

    EmuFault,
    AlignFault,

    CtxSwitch,
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/d0d1dd628527c77db2391ce0293c1ed344b2365f>
    CgroupSwitch,

    Dummy,
    /// Since `linux-4.4`: <https://github.com/torvalds/linux/commit/a43eec304259a6c637f4014a6d4767159b6a3aa3>
    BpfOutput,
    CpuMigration,
}

super::try_from!(Software, value, {
    #[rustfmt::skip]
    let config =  match value {
        Software::CpuClock       => b::PERF_COUNT_SW_CPU_CLOCK,
        Software::TaskClock      => b::PERF_COUNT_SW_TASK_CLOCK,

        Software::PageFault      => b::PERF_COUNT_SW_PAGE_FAULTS,
        Software::MinorPageFault => b::PERF_COUNT_SW_PAGE_FAULTS_MIN,
        Software::MajorPageFault => b::PERF_COUNT_SW_PAGE_FAULTS_MAJ,

        Software::EmuFault => b::PERF_COUNT_SW_EMULATION_FAULTS,
        Software::AlignFault => b::PERF_COUNT_SW_ALIGNMENT_FAULTS,

        Software::CtxSwitch      => b::PERF_COUNT_SW_CONTEXT_SWITCHES,
        #[cfg(feature="linux-5.13")]
        Software::CgroupSwitch   => b::PERF_COUNT_SW_CGROUP_SWITCHES,

        Software::Dummy          => b::PERF_COUNT_SW_DUMMY,
        #[cfg(feature="linux-4.4")]
        Software::BpfOutput      => b::PERF_COUNT_SW_BPF_OUTPUT,
        Software::CpuMigration   => b::PERF_COUNT_SW_CPU_MIGRATIONS,

        #[cfg(not(feature="linux-5.13"))]
        _  => crate::config::unsupported!(),
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
