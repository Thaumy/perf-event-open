use super::EventConfig;
use crate::ffi::bindings as b;

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

super::try_from!(Hardware, value, {
    let config = match value {
        Hardware::CpuCycle => b::PERF_COUNT_HW_CPU_CYCLES as _,
        Hardware::BusCycle => b::PERF_COUNT_HW_BUS_CYCLES as _,
        Hardware::RefCpuCycle => b::PERF_COUNT_HW_REF_CPU_CYCLES as _,

        Hardware::Cache(ty, op, result) => {
            let id = match ty {
                Type::L1d => b::PERF_COUNT_HW_CACHE_L1D,
                Type::L1i => b::PERF_COUNT_HW_CACHE_L1I,
                Type::Ll => b::PERF_COUNT_HW_CACHE_LL,
                Type::Dtlb => b::PERF_COUNT_HW_CACHE_DTLB,
                Type::Itlb => b::PERF_COUNT_HW_CACHE_ITLB,
                Type::Bpu => b::PERF_COUNT_HW_CACHE_BPU,
                Type::Node => b::PERF_COUNT_HW_CACHE_NODE,
            } as u64;
            let op = match op {
                Op::Read => b::PERF_COUNT_HW_CACHE_OP_READ,
                Op::Write => b::PERF_COUNT_HW_CACHE_OP_WRITE,
                Op::Prefetch => b::PERF_COUNT_HW_CACHE_OP_PREFETCH,
            } as u64;
            let op_result = match result {
                OpResult::Miss => b::PERF_COUNT_HW_CACHE_RESULT_MISS,
                OpResult::Access => b::PERF_COUNT_HW_CACHE_RESULT_ACCESS,
            } as u64;
            id | (op << 8) | (op_result << 16)
        }

        Hardware::CacheMiss => b::PERF_COUNT_HW_CACHE_MISSES as _,
        Hardware::CacheAccess => b::PERF_COUNT_HW_CACHE_REFERENCES as _,

        Hardware::BranchMiss => b::PERF_COUNT_HW_BRANCH_MISSES as _,
        Hardware::BranchInstr => b::PERF_COUNT_HW_BRANCH_INSTRUCTIONS as _,

        Hardware::BackendStalledCycle => b::PERF_COUNT_HW_STALLED_CYCLES_BACKEND as _,
        Hardware::FrontendStalledCycle => b::PERF_COUNT_HW_STALLED_CYCLES_FRONTEND as _,

        Hardware::Instr => b::PERF_COUNT_HW_INSTRUCTIONS as _,
    };

    let event_config = EventConfig {
        ty: b::PERF_TYPE_HARDWARE,
        config,
        config1: 0,
        config2: 0,
        config3: 0,
        bp_type: 0,
    };

    Ok(Self(event_config))
});
