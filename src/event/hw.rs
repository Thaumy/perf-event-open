#[derive(Clone, Debug)]
pub enum Hardware {
    CpuCycle,
    BusCycle,
    RefCpuCycle,

    Cache(Type, Op, OpResult),
    CacheMiss,
    CacheAccess,

    BranchMiss,
    BranchInstr,

    BackendStalledCycle,
    FrontendStalledCycle,

    Instr,
}

#[derive(Clone, Debug)]
pub enum Type {
    L1d,
    L1i,
    Ll,
    Dtlb,
    Itlb,
    Bpu,
    Node,
}

#[derive(Clone, Debug)]
pub enum Op {
    Read,
    Write,
    Prefetch,
}

#[derive(Clone, Debug)]
pub enum OpResult {
    Miss,
    Access,
}
