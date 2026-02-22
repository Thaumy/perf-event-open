use super::EventConfig;
use crate::ffi::bindings as b;

/// Generalized hardware CPU events.
///
/// Not all of these are available on all platforms.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Hardware {
    /// Total CPU cycles, affected by CPU frequency scaling.
    CpuCycle,
    /// Bus cycles, which can be different from total cycles.
    BusCycle,
    /// Reference CPU cycles, not affected by CPU frequency scaling.
    RefCpuCycle,

    /// Hardware CPU cache event.
    Cache(Type, Op, OpResult),
    /// Cache misses.
    ///
    /// Usually this indicates Last Level Cache misses; this is intended to be used
    /// in conjunction with the `CacheAccess` event to calculate cache miss rates
    /// ([cache miss][Self::CacheMiss] / [cache access][Self::CacheAccess]).
    CacheMiss,
    /// Cache accesses.
    ///
    /// Usually this indicates Last Level Cache accesses but this may vary
    /// depending on your CPU. This may include prefetches and coherency messages;
    /// again this depends on the design of your CPU.
    CacheAccess,

    /// Mispredicted branch instructions.
    BranchMiss,
    /// Branch instructions retired.
    BranchInstr,

    /// Stalled cycles during issue.
    BackendStalledCycle,
    /// Stalled cycles during retirement.
    FrontendStalledCycle,

    /// Retired instructions.
    ///
    /// Be careful, these can be affected by various issues, most notably
    /// hardware interrupt counts.
    Instr,
}

/// Type of cache
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    /// L1 data cache.
    L1d,
    /// L1 instruction cache.
    L1i,
    /// Last-level cache.
    Ll,
    /// Data TLB.
    Dtlb,
    /// Instruction TLB.
    Itlb,
    /// Branch prediction unit.
    Bpu,
    /// Local memory accesses.
    Node,
}

/// Cache operations
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Op {
    /// Read accesses.
    Read,
    /// Write accesses.
    Write,
    /// Prefetch accesses.
    Prefetch,
}

/// Cache operation results.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OpResult {
    /// Operation misses.
    Miss,
    /// Operation accesses.
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
