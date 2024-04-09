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
    CgroupSwitch,

    Dummy,
    BpfOutput,
    CpuMigration,
}
