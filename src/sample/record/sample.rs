use std::slice;

use super::{RecordId, Task};
use crate::count::Stat;
use crate::ffi::{bindings as b, deref_offset};

/// Sample.
///
/// Fields can be enabled via [`SampleFormat`][crate::config::SampleFormat].
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sample {
    /// Record IDs.
    pub record_id: RecordId,

    /// Counter statistics.
    pub stat: Option<Stat>,
    /// Sampling period.
    pub period: Option<u64>,
    /// Cgroup ID (for the perf event subsystem).
    ///
    /// To get the pathname of the cgroup, the ID should match to
    /// [`Cgroup::id`][crate::sample::record::Cgroup::id].
    pub cgroup: Option<u64>,
    /// Call chain (stack backtrace).
    pub call_chain: Option<Vec<u64>>,
    /// User stack.
    pub user_stack: Option<Vec<u8>>,

    /// Data address.
    ///
    /// This is usually the address of a tracepoint, breakpoint, or software event;
    /// otherwise the value is 0.
    pub data_addr: Option<u64>,
    /// Physical data address.
    ///
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/fc7ce9c74c3ad232b084d80148654f926d01ece7>
    pub data_phys_addr: Option<u64>,
    /// Page size of [data address][Self::data_addr].
    ///
    /// Since `linux-5.11`: <https://github.com/torvalds/linux/commit/8d97e71811aaafe4abf611dc24822fd6e73df1a1>
    pub data_page_size: Option<u64>,
    /// The source of data associated with the sampled instruction.
    pub data_source: Option<DataSource>,

    /// Code address (instruction pointer).
    ///
    /// The second member will be true if the instruction pointer points to the actual
    /// instruction that triggered the event (0 [skid][crate::config::SampleSkid]).
    pub code_addr: Option<(u64, bool)>,
    /// Page size of [code address][Self::code_addr].
    ///
    /// Since `linux-5.11`: <https://github.com/torvalds/linux/commit/995f088efebe1eba0282a6ffa12411b37f8990c2>
    pub code_page_size: Option<u64>,

    /// Registers at sample time.
    pub user_regs: Option<(Vec<u64>, Abi)>,
    /// Registers at interrupt (event overflow).
    pub intr_regs: Option<(Vec<u64>, Abi)>,

    /// Raw data.
    ///
    /// This raw data is opaque with respect to the ABI. The ABI doesn't
    /// make any promises with respect to the stability of its content,
    /// it may vary depending on event, hardware, and kernel version.
    pub raw: Option<Vec<u8>>,
    /// LBR data.
    ///
    /// This provides a record of recent branches, as provided by
    /// CPU branch sampling hardware (such as Intel LBR).
    ///
    /// Not all hardware supports this feature.
    pub lbr: Option<Lbr>,
    /// A snapshot of the AUX area.
    pub aux: Option<Vec<u8>>,
    /// The sources of any transactional memory aborts.
    pub txn: Option<Txn>,
    /// A hardware provided [weight][crate::sample::record::sample::Sample::weight]
    /// value that expresses how costly the sampled event was.
    ///
    /// This allows the hardware to highlight expensive events in a profile.
    pub weight: Option<Weight>,
}

impl Sample {
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L957
    // struct {
    //     struct perf_event_header header;
    //     { u64 id;        } && PERF_SAMPLE_IDENTIFIER
    //     { u64 ip;        } && PERF_SAMPLE_IP
    //     { u32 pid, tid;  } && PERF_SAMPLE_TID
    //     { u64 time;      } && PERF_SAMPLE_TIME
    //     { u64 addr;      } && PERF_SAMPLE_ADDR
    //     { u64 id;        } && PERF_SAMPLE_ID
    //     { u64 stream_id; } && PERF_SAMPLE_STREAM_ID
    //     { u32 cpu, res;  } && PERF_SAMPLE_CPU
    //     { u64 period;    } && PERF_SAMPLE_PERIOD
    //     { struct read_format values; } && PERF_SAMPLE_READ
    //     {
    //         u64 nr,
    //         u64 ips[nr];
    //     } && PERF_SAMPLE_CALLCHAIN
    //     {
    //         u32 size;
    //         char data[size];
    //     } && PERF_SAMPLE_RAW
    //     {
    //         u64 nr;
    //         { u64 hw_idx;         } && PERF_SAMPLE_BRANCH_HW_INDEX
    //         { u64 from, to, flags } lbr[nr];
    //         { u64 counters;       } cntr[nr] && PERF_SAMPLE_BRANCH_COUNTERS
    //     } && PERF_SAMPLE_BRANCH_STACK
    //     {
    //         u64 abi; # enum perf_sample_regs_abi
    //         u64 regs[weight(mask)];
    //     } && PERF_SAMPLE_REGS_USER
    //     {
    //         u64 size;
    //         char data[size];
    //         u64 dyn_size;
    //     } && PERF_SAMPLE_STACK_USER
    //     union perf_sample_weight {
    //         u64 full; && PERF_SAMPLE_WEIGHT
    //         #if defined(__LITTLE_ENDIAN_BITFIELD)
    //         struct {
    //             u32 var1_dw;
    //             u16 var2_w;
    //             u16 var3_w;
    //         } && PERF_SAMPLE_WEIGHT_STRUCT
    //         #elif defined(__BIG_ENDIAN_BITFIELD)
    //         struct {
    //             u16 var3_w;
    //             u16 var2_w;
    //             u32 var1_dw;
    //         } && PERF_SAMPLE_WEIGHT_STRUCT
    //         #endif
    //     }
    //     { u64 data_src;    } && PERF_SAMPLE_DATA_SRC
    //     { u64 transaction; } && PERF_SAMPLE_TRANSACTION
    //     {
    //         u64 abi; # enum perf_sample_regs_abi
    //         # https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7620
    //         { u64 regs[weight(mask)]; } # if abi != 0
    //     } && PERF_SAMPLE_REGS_INTR
    //     { u64 phys_addr; } && PERF_SAMPLE_PHYS_ADDR
    //     # https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7632
    //     { u64 cgroup; } && PERF_SAMPLE_CGROUP
    //     # About the order:
    //     # https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7635
    //     { u64 data_page_size; } && PERF_SAMPLE_DATA_PAGE_SIZE
    //     { u64 code_page_size; } && PERF_SAMPLE_CODE_PAGE_SIZE
    //     {
    //         u64 size;
    //         char data[size];
    //     } && PERF_SAMPLE_AUX
    // };
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        misc: u16,
        read_format: u64,
        sample_type: u64,
        user_regs: usize,
        intr_regs: usize,
        branch_sample_type: u64,
    ) -> Self {
        macro_rules! when {
            ($($feature: literal,)? $flag:ident, $ty:ty) => {{
                $(#[cfg(feature = $feature)])?
                let val = (sample_type & (b::$flag as u64) > 0).then(|| deref_offset::<$ty>(&mut ptr));
                $(
                #[cfg(not(feature = $feature))]
                let val = None;
                )?
                val
            }};
            ($flag:ident) => {
                sample_type & (b::$flag as u64) > 0
            };
            ($($feature: literal,)? $flag:ident, $then:expr) => {{
                $(#[cfg(feature = $feature)])?
                let val = (sample_type & (b::$flag as u64) > 0).then(|| $then);
                $(
                #[cfg(not(feature = $feature))]
                let val = None;
                )?
                val
            }};
        }

        // For `PERF_SAMPLE_IDENTIFIER`:
        // `PERF_SAMPLE_IDENTIFIER` just duplicates the `PERF_SAMPLE_ID` at a fixed offset,
        // it's useful to distinguish the sample format if multiple events share the same rb.
        // Our design does not support redirecting samples to another rb (e.g., `PERF_FLAG_FD_OUTPUT`),
        // and this is not a parser crate, so `PERF_SAMPLE_IDENTIFIER` is not needed.
        // See:
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7342
        // https://github.com/torvalds/linux/blob/v6.13/tools/perf/Documentation/perf.data-file-format.txt#L466
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12808

        let code_addr = when!(PERF_SAMPLE_IP, {
            (
                deref_offset(&mut ptr),
                misc as u32 & b::PERF_RECORD_MISC_EXACT_IP > 0,
            )
        });
        let task = when!(
            PERF_SAMPLE_TID,
            Task {
                pid: deref_offset(&mut ptr),
                tid: deref_offset(&mut ptr),
            }
        );
        let time = when!(PERF_SAMPLE_TIME, u64);
        let data_addr = when!(PERF_SAMPLE_ADDR, u64);
        let id = when!(PERF_SAMPLE_ID, u64);
        let stream_id = when!(PERF_SAMPLE_STREAM_ID, u64);
        let cpu = when!(PERF_SAMPLE_CPU, {
            let val = deref_offset(&mut ptr);
            ptr = ptr.add(size_of::<u32>());
            val
        });
        let period = when!(PERF_SAMPLE_PERIOD, u64);
        let stat = when!(PERF_SAMPLE_READ, {
            Stat::from_ptr_offset(&mut ptr, read_format)
        });
        let call_chain = when!(PERF_SAMPLE_CALLCHAIN, {
            let len = deref_offset::<u64>(&mut ptr) as usize;
            let ips = slice::from_raw_parts(ptr as *const u64, len);
            ptr = ptr.add(len * size_of::<u64>());
            ips.to_vec()
        });
        let raw = when!(PERF_SAMPLE_RAW, {
            let len = deref_offset::<u32>(&mut ptr) as usize;
            let bytes = slice::from_raw_parts(ptr, len);
            ptr = ptr.add(len);
            // https://github.com/torvalds/linux/blob/v6.13/include/linux/perf_event.h#L1303
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            bytes.to_vec()
        });
        let lbr = when!(PERF_SAMPLE_BRANCH_STACK, {
            parse_lbr(&mut ptr, branch_sample_type)
        })
        .flatten();
        let user_regs = when!(PERF_SAMPLE_REGS_USER, { parse_regs(&mut ptr, user_regs) }).flatten();
        let user_stack = when!(PERF_SAMPLE_STACK_USER, {
            let len = deref_offset::<u64>(&mut ptr) as usize;
            let bytes = slice::from_raw_parts(ptr, len);
            ptr = ptr.add(len);
            let dyn_len = if len > 0 {
                deref_offset::<u64>(&mut ptr) as usize
            } else {
                0
            };
            bytes[..dyn_len].to_vec()
        });
        #[cfg(feature = "linux-5.12")]
        let weight = if when!(PERF_SAMPLE_WEIGHT) {
            let full = Weight::Full(deref_offset(&mut ptr));
            Some(full)
        } else if when!(PERF_SAMPLE_WEIGHT_STRUCT) {
            #[cfg(target_endian = "little")]
            let vars = Weight::Vars {
                var1: deref_offset(&mut ptr),
                var2: deref_offset(&mut ptr),
                var3: deref_offset(&mut ptr),
            };
            #[cfg(target_endian = "big")]
            let vars = Weight::Vars {
                var3: deref_offset(&mut ptr),
                var2: deref_offset(&mut ptr),
                var1: deref_offset(&mut ptr),
            };
            Some(vars)
        } else {
            None
        };
        #[cfg(not(feature = "linux-5.12"))]
        let weight = when!(PERF_SAMPLE_WEIGHT, { Weight::Full(deref_offset(&mut ptr)) });
        let data_source = when!(PERF_SAMPLE_DATA_SRC, { parse_data_source(&mut ptr) });
        let txn = when!(PERF_SAMPLE_TRANSACTION, { parse_txn(&mut ptr) });
        let intr_regs = when!(PERF_SAMPLE_REGS_INTR, { parse_regs(&mut ptr, intr_regs) }).flatten();
        let data_phys_addr = when!("linux-4.14", PERF_SAMPLE_PHYS_ADDR, u64);
        let cgroup = when!("linux-5.7", PERF_SAMPLE_CGROUP, u64);
        let data_page_size = when!("linux-5.11", PERF_SAMPLE_DATA_PAGE_SIZE, u64);
        let code_page_size = when!("linux-5.11", PERF_SAMPLE_CODE_PAGE_SIZE, u64);
        let aux = when!("linux-5.5", PERF_SAMPLE_AUX, {
            let len = deref_offset::<u64>(&mut ptr) as usize;
            let bytes = slice::from_raw_parts(ptr, len as _);
            bytes.to_vec()
        });

        Self {
            record_id: RecordId {
                id,
                stream_id,
                cpu,
                task,
                time,
            },

            stat,
            period,
            cgroup,
            call_chain,
            user_stack,

            data_addr,
            data_phys_addr,
            data_page_size,
            data_source,

            code_addr,
            code_page_size,

            user_regs,
            intr_regs,

            raw,
            lbr,
            aux,
            txn,
            weight,
        }
    }
}

super::from!(Sample);

super::debug!(Sample {
    {record_id},
    {stat?},
    {period?},
    {cgroup?},
    {call_chain?},
    {user_stack?},
    {data_source?},
    {data_addr?},
    {data_phys_addr?},
    {data_page_size?},
    {code_addr?},
    {code_page_size?},
    {user_regs?},
    {intr_regs?},
    {raw?},
    {lbr?},
    {aux?},
    {txn?},
    {weight?},
});

unsafe fn parse_regs(ptr: &mut *const u8, len: usize) -> Option<(Vec<u64>, Abi)> {
    let abi = deref_offset::<u64>(ptr) as u32;

    // PERF_SAMPLE_REGS_USER: https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7589
    // PERF_SAMPLE_REGS_INTR: https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7620
    if abi == b::PERF_SAMPLE_REGS_ABI_NONE {
        return None;
    }

    let regs = slice::from_raw_parts(*ptr as *const u64, len);
    *ptr = ptr.add(len * size_of::<u64>());
    let abi = match abi {
        b::PERF_SAMPLE_REGS_ABI_32 => Abi::_32,
        b::PERF_SAMPLE_REGS_ABI_64 => Abi::_64,
        _ => unimplemented!(),
    };

    Some((regs.to_vec(), abi))
}

unsafe fn parse_lbr(ptr: &mut *const u8, branch_sample_type: u64) -> Option<Lbr> {
    let len = deref_offset::<u64>(ptr) as usize;
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7575
    if len == 0 {
        return None;
    }

    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L7560
    #[cfg(feature = "linux-5.7")]
    let hw_index = (branch_sample_type & b::PERF_SAMPLE_BRANCH_HW_INDEX as u64 > 0)
        .then(|| deref_offset::<u64>(ptr));
    #[cfg(not(feature = "linux-5.7"))]
    let _ = branch_sample_type;
    #[cfg(not(feature = "linux-5.7"))]
    let hw_index = None;

    #[repr(C)]
    struct Layout {
        from: u64,
        to: u64,
        bits: u64,
    }
    fn to_entry(layout: &Layout, counter: Option<u64>) -> Entry {
        let bits = layout.bits;

        macro_rules! when {
            ($flag:expr) => {
                bits & $flag > 0
            };
        }

        Entry {
            counter,

            from: layout.from,
            to: layout.to,

            // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1439
            mis: when!(0b1),          // 0, 1 bit
            pred: when!(0b10),        // 1, 1 bit
            in_tx: when!(0b100),      // 2, 1 bit
            abort: when!(0b1000),     // 3, 1 bit
            cycles: (bits >> 4) as _, // 4-19, 16 bits
            #[cfg(feature = "linux-4.14")]
            // 20-23, 4 bits
            branch_type: match ((bits >> 20) & 0b1111) as _ {
                b::PERF_BR_UNKNOWN => BranchType::Unknown,
                b::PERF_BR_COND => BranchType::Cond,
                b::PERF_BR_UNCOND => BranchType::Uncond,
                b::PERF_BR_IND => BranchType::Ind,
                b::PERF_BR_CALL => BranchType::Call,
                b::PERF_BR_IND_CALL => BranchType::IndCall,
                b::PERF_BR_RET => BranchType::Ret,
                b::PERF_BR_SYSCALL => BranchType::Syscall,
                b::PERF_BR_SYSRET => BranchType::Sysret,
                b::PERF_BR_COND_CALL => BranchType::CondCall,
                b::PERF_BR_COND_RET => BranchType::CondRet,
                #[cfg(feature = "linux-5.18")]
                b::PERF_BR_ERET => BranchType::Eret,
                #[cfg(feature = "linux-5.18")]
                b::PERF_BR_IRQ => BranchType::Irq,
                #[cfg(feature = "linux-6.1")]
                b::PERF_BR_SERROR => BranchType::SysErr,
                #[cfg(feature = "linux-6.1")]
                b::PERF_BR_NO_TX => BranchType::NoTx,
                #[cfg(feature = "linux-6.1")]
                // match new_type
                // https://github.com/torvalds/linux/blob/v6.13/tools/perf/util/branch.c#L106
                b::PERF_BR_EXTEND_ABI => match ((bits >> 26) & 0b1111) as _ {
                    b::PERF_BR_NEW_FAULT_DATA => BranchType::DataFault,
                    b::PERF_BR_NEW_FAULT_ALGN => BranchType::AlignFault,
                    b::PERF_BR_NEW_FAULT_INST => BranchType::InstrFault,
                    b::PERF_BR_NEW_ARCH_1 => BranchType::Arch1,
                    b::PERF_BR_NEW_ARCH_2 => BranchType::Arch2,
                    b::PERF_BR_NEW_ARCH_3 => BranchType::Arch3,
                    b::PERF_BR_NEW_ARCH_4 => BranchType::Arch4,
                    b::PERF_BR_NEW_ARCH_5 => BranchType::Arch5,
                    // For compatibility, not ABI.
                    _ => BranchType::Unknown,
                },
                // For compatibility, not ABI.
                _ => BranchType::Unknown,
            },
            #[cfg(not(feature = "linux-4.14"))]
            branch_type: BranchType::Unknown,
            #[cfg(feature = "linux-6.1")]
            // 24-25, 2 bits
            branch_spec: match ((bits >> 24) & 0b11) as _ {
                b::PERF_BR_SPEC_NA => BranchSpec::Na,
                b::PERF_BR_SPEC_WRONG_PATH => BranchSpec::Wrong,
                b::PERF_BR_NON_SPEC_CORRECT_PATH => BranchSpec::NoSpecCorrect,
                b::PERF_BR_SPEC_CORRECT_PATH => BranchSpec::Correct,
                _ => unreachable!(),
            },
            #[cfg(not(feature = "linux-6.1"))]
            branch_spec: BranchSpec::Na,
            // new_type: 26-29, 4 bits
            #[cfg(feature = "linux-6.1")]
            // 30-32, 3 bits
            branch_priv: match ((bits >> 30) & 0b111) as _ {
                b::PERF_BR_PRIV_UNKNOWN => BranchPriv::Unknown,
                b::PERF_BR_PRIV_USER => BranchPriv::User,
                b::PERF_BR_PRIV_KERNEL => BranchPriv::Kernel,
                b::PERF_BR_PRIV_HV => BranchPriv::Hv,
                // For compatibility, not ABI.
                _ => BranchPriv::Unknown,
            },
            #[cfg(not(feature = "linux-6.1"))]
            branch_priv: BranchPriv::Unknown,
            // reserved: 33-63, 31 bits
        }
    }

    let layouts = slice::from_raw_parts(*ptr as *const Layout, len).iter();
    // https://github.com/torvalds/linux/commit/571d91dcadfa3cef499010b4eddb9b58b0da4d24
    #[cfg(feature = "linux-6.8")]
    let has_counters = branch_sample_type & b::PERF_SAMPLE_BRANCH_COUNTERS as u64 > 0;
    #[cfg(not(feature = "linux-6.8"))]
    let has_counters = false;
    let entries = if has_counters {
        *ptr = ptr.add(len * size_of::<Layout>());
        layouts
            .map(|it| to_entry(it, Some(deref_offset(ptr))))
            .collect()
    } else {
        layouts.map(|it| to_entry(it, None)).collect()
    };

    Some(Lbr { hw_index, entries })
}

unsafe fn parse_txn(ptr: &mut *const u8) -> Txn {
    let bits: u64 = deref_offset(ptr);
    let code = ((bits & b::PERF_TXN_ABORT_MASK) >> b::PERF_TXN_ABORT_SHIFT) as u32;
    macro_rules! when {
        ($flag:ident) => {
            bits & b::$flag > 0
        };
    }
    Txn {
        elision: when!(PERF_TXN_ELISION),
        tx: when!(PERF_TXN_TRANSACTION),
        is_sync: when!(PERF_TXN_SYNC),
        is_async: when!(PERF_TXN_ASYNC),
        retry: when!(PERF_TXN_RETRY),
        conflict: when!(PERF_TXN_CONFLICT),
        capacity_read: when!(PERF_TXN_CAPACITY_READ),
        capacity_write: when!(PERF_TXN_CAPACITY_WRITE),
        code,
    }
}

unsafe fn parse_data_source(ptr: &mut *const u8) -> DataSource {
    let bits: u64 = deref_offset(ptr);

    // u64 (little-endian):
    // mem_op        0-4  5 bits, type of opcode
    // mem_lvl      5-18 14 bits, memory hierarchy level
    // mem_snoop   19-23  5 bits, snoop mode
    // mem_lock    24-25  2 bits, lock instr
    // mem_dtlb    26-32  7 bits, tlb access
    // mem_lvl_num 33-36  4 bits, memory hierarchy level number
    // mem_remote     37  1 bit,  remote
    // mem_snoopx  38-39  2 bits, snoop mode, ext
    // mem_blk     40-42  3 bits, access blocked
    // mem_hops    43-45  3 bits, hop level
    // mem_rsvd    46-63 18 bits, reserved

    macro_rules! when {
        ($shifted:expr, $flag:ident) => {
            $shifted & (b::$flag as u64) > 0
        };
    }

    let op = MemOp {
        na: when!(bits, PERF_MEM_OP_NA),
        load: when!(bits, PERF_MEM_OP_LOAD),
        store: when!(bits, PERF_MEM_OP_STORE),
        prefetch: when!(bits, PERF_MEM_OP_PFETCH),
        exec: when!(bits, PERF_MEM_OP_EXEC),
    };

    let shifted = bits >> b::PERF_MEM_LVL_SHIFT;
    let level = MemLevel {
        na: when!(shifted, PERF_MEM_LVL_NA),
        hit: when!(shifted, PERF_MEM_LVL_HIT),
        miss: when!(shifted, PERF_MEM_LVL_MISS),
        l1: when!(shifted, PERF_MEM_LVL_L1),
        lfb: when!(shifted, PERF_MEM_LVL_LFB),
        l2: when!(shifted, PERF_MEM_LVL_L2),
        l3: when!(shifted, PERF_MEM_LVL_L3),
        loc_ram: when!(shifted, PERF_MEM_LVL_LOC_RAM),
        rem_ram1: when!(shifted, PERF_MEM_LVL_REM_RAM1),
        rem_ram2: when!(shifted, PERF_MEM_LVL_REM_RAM2),
        rem_cce1: when!(shifted, PERF_MEM_LVL_REM_CCE1),
        rem_cce2: when!(shifted, PERF_MEM_LVL_REM_CCE2),
        io: when!(shifted, PERF_MEM_LVL_IO),
        unc: when!(shifted, PERF_MEM_LVL_UNC),
    };

    let shifted1 = bits >> b::PERF_MEM_SNOOP_SHIFT;
    #[cfg(feature = "linux-4.14")]
    let shifted2 = bits >> b::PERF_MEM_SNOOPX_SHIFT;
    let snoop = MemSnoop {
        na: when!(shifted1, PERF_MEM_SNOOP_NA),
        none: when!(shifted1, PERF_MEM_SNOOP_NONE),
        hit: when!(shifted1, PERF_MEM_SNOOP_HIT),
        miss: when!(shifted1, PERF_MEM_SNOOP_MISS),
        hit_m: when!(shifted1, PERF_MEM_SNOOP_HITM),
        #[cfg(feature = "linux-4.14")]
        fwd: when!(shifted2, PERF_MEM_SNOOPX_FWD),
        #[cfg(not(feature = "linux-4.14"))]
        fwd: false,
        #[cfg(feature = "linux-6.1")]
        peer: when!(shifted2, PERF_MEM_SNOOPX_PEER),
        #[cfg(not(feature = "linux-6.1"))]
        peer: false,
    };

    let shifted = bits >> b::PERF_MEM_LOCK_SHIFT;
    let lock = MemLock {
        na: when!(shifted, PERF_MEM_LOCK_NA),
        locked: when!(shifted, PERF_MEM_LOCK_LOCKED),
    };

    let shifted = bits >> b::PERF_MEM_TLB_SHIFT;
    let tlb = MemTlb {
        na: when!(shifted, PERF_MEM_TLB_NA),
        hit: when!(shifted, PERF_MEM_TLB_HIT),
        miss: when!(shifted, PERF_MEM_TLB_MISS),
        l1: when!(shifted, PERF_MEM_TLB_L1),
        l2: when!(shifted, PERF_MEM_TLB_L2),
        walker: when!(shifted, PERF_MEM_TLB_WK),
        fault: when!(shifted, PERF_MEM_TLB_OS),
    };

    #[cfg(feature = "linux-4.14")]
    let shifted = bits >> b::PERF_MEM_LVLNUM_SHIFT;
    #[cfg(feature = "linux-4.14")]
    let level2 = match (shifted & 0b1111) as u32 {
        b::PERF_MEM_LVLNUM_L1 => MemLevel2::L1,
        b::PERF_MEM_LVLNUM_L2 => MemLevel2::L2,
        b::PERF_MEM_LVLNUM_L3 => MemLevel2::L3,
        b::PERF_MEM_LVLNUM_L4 => MemLevel2::L4,
        #[cfg(feature = "linux-6.11")]
        b::PERF_MEM_LVLNUM_L2_MHB => MemLevel2::L2Mhb,
        #[cfg(feature = "linux-6.11")]
        b::PERF_MEM_LVLNUM_MSC => MemLevel2::Msc,
        #[cfg(feature = "linux-6.6")]
        b::PERF_MEM_LVLNUM_UNC => MemLevel2::Unc,
        #[cfg(feature = "linux-6.1")]
        b::PERF_MEM_LVLNUM_CXL => MemLevel2::Cxl,
        #[cfg(feature = "linux-6.1")]
        b::PERF_MEM_LVLNUM_IO => MemLevel2::Io,
        b::PERF_MEM_LVLNUM_ANY_CACHE => MemLevel2::AnyCache,
        b::PERF_MEM_LVLNUM_LFB => MemLevel2::Lfb,
        b::PERF_MEM_LVLNUM_RAM => MemLevel2::Ram,
        b::PERF_MEM_LVLNUM_PMEM => MemLevel2::Pmem,
        b::PERF_MEM_LVLNUM_NA => MemLevel2::Na,
        // For compatibility, not ABI.
        _ => MemLevel2::Unknown,
    };
    #[cfg(not(feature = "linux-4.14"))]
    let level2 = MemLevel2::Unknown;

    #[cfg(feature = "linux-4.14")]
    let remote = (bits >> b::PERF_MEM_REMOTE_SHIFT) & 1 > 0;
    #[cfg(not(feature = "linux-4.14"))]
    let remote = false;

    #[cfg(feature = "linux-5.12")]
    let shifted = bits >> b::PERF_MEM_BLK_SHIFT;
    #[cfg(feature = "linux-5.12")]
    let block = MemBlock {
        na: when!(shifted, PERF_MEM_BLK_NA),
        data: when!(shifted, PERF_MEM_BLK_DATA),
        addr: when!(shifted, PERF_MEM_BLK_ADDR),
    };
    #[cfg(not(feature = "linux-5.12"))]
    let block = MemBlock {
        na: false,
        data: false,
        addr: false,
    };

    #[cfg(feature = "linux-5.16")]
    let shifted = bits >> b::PERF_MEM_HOPS_SHIFT;
    #[cfg(feature = "linux-5.16")]
    let hops = match (shifted & 0b111) as u32 {
        b::PERF_MEM_HOPS_0 => MemHop::Core,
        #[cfg(feature = "linux-5.17")]
        b::PERF_MEM_HOPS_1 => MemHop::Node,
        #[cfg(feature = "linux-5.17")]
        b::PERF_MEM_HOPS_2 => MemHop::Socket,
        #[cfg(feature = "linux-5.17")]
        b::PERF_MEM_HOPS_3 => MemHop::Board,
        // For compatibility, not ABI.
        _ => MemHop::Unknown,
    };
    #[cfg(not(feature = "linux-5.16"))]
    let hops = MemHop::Unknown;

    DataSource {
        op,
        level,
        snoop,
        lock,
        tlb,
        level2,
        remote,
        block,
        hops,
    }
}

/// LBR data.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lbr {
    /// The index in the underlying hardware buffer of the most recently
    /// captured taken branch.
    ///
    /// It is very useful for reconstructing the call stack.
    /// For example, in Intel LBR call stack mode, the depth of reconstructed
    /// LBR call stack limits to the number of LBR registers. With the low level
    /// index information, perf tool may stitch the stacks of two samples.
    /// The reconstructed LBR call stack can break the hardware limitation.
    ///
    /// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/bbfd5e4fab63703375eafaf241a0c696024a59e1>
    pub hw_index: Option<u64>,

    /// LBR entries.
    pub entries: Vec<Entry>,
}

super::debug!(Lbr {
    {hw_index?},
    {entries},
});

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1436
/// LBR entry.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    pub from: u64,
    pub to: u64,

    pub mis: bool,
    pub pred: bool,
    pub in_tx: bool,
    pub abort: bool,
    pub cycles: u16,

    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/eb0baf8a0d9259d168523b8e7c436b55ade7c546>
    pub branch_type: BranchType,
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/93315e46b000fc80fff5d53c3f444417fb3df6de>
    pub branch_spec: BranchSpec,
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/5402d25aa5710d240040f73fb13d7d5c303ef071>
    pub branch_priv: BranchPriv,

    // https://github.com/torvalds/linux/commit/571d91dcadfa3cef499010b4eddb9b58b0da4d24
    /// This counter may store the occurrences of several events.
    ///
    /// Since `linux-6.8`: <https://github.com/torvalds/linux/commit/571d91dcadfa3cef499010b4eddb9b58b0da4d24>
    pub counter: Option<u64>,
}

/// Branch types.
///
/// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/eb0baf8a0d9259d168523b8e7c436b55ade7c546>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BranchType {
    // PERF_BR_*
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L248

    // PERF_BR_UNKNOWN
    /// Unknown.
    Unknown,
    // PERF_BR_COND
    /// Conditional.
    Cond,
    // PERF_BR_UNCOND
    /// Unconditional.
    Uncond,
    // PERF_BR_IND
    /// Indirect.
    Ind,
    // PERF_BR_CALL
    /// Function call.
    Call,
    // PERF_BR_IND_CALL
    /// Indirect function call.
    IndCall,
    // PERF_BR_RET
    /// Function return.
    Ret,
    // PERF_BR_SYSCALL
    /// Syscall.
    Syscall,
    // PERF_BR_SYSRET
    /// Syscall return.
    Sysret,
    // PERF_BR_COND_CALL
    /// Conditional function call.
    CondCall,
    // PERF_BR_COND_RET
    /// Conditional function return.
    CondRet,
    // PERF_BR_ERET
    /// Exception return.
    /// Since `linux-5.18`: <https://github.com/torvalds/linux/commit/cedd3614e5d9c80908099c19f8716714ce0610b1>
    Eret,
    // PERF_BR_IRQ
    /// IRQ.
    /// Since `linux-5.18`: <https://github.com/torvalds/linux/commit/cedd3614e5d9c80908099c19f8716714ce0610b1>
    Irq,
    // PERF_BR_SERROR
    /// System error.
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/a724ec82966d57e4b5d36341d3e3dc1a3c011564>
    SysErr,
    // PERF_BR_NO_TX
    /// Not in transaction.
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/a724ec82966d57e4b5d36341d3e3dc1a3c011564>
    NoTx,

    // PERF_BR_NEW_*
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L279

    // PERF_BR_NEW_FAULT_DATA
    /// Data fault.
    DataFault,
    // PERF_BR_NEW_FAULT_ALGN
    /// Alignment fault.
    AlignFault,
    // PERF_BR_NEW_FAULT_INST
    /// Instruction fault.
    InstrFault,

    // PERF_BR_NEW_ARCH_1
    /// Architecture specific.
    Arch1,
    // PERF_BR_NEW_ARCH_2
    /// Architecture specific.
    Arch2,
    // PERF_BR_NEW_ARCH_3
    /// Architecture specific.
    Arch3,
    // PERF_BR_NEW_ARCH_4
    /// Architecture specific.
    Arch4,
    // PERF_BR_NEW_ARCH_5
    /// Architecture specific.
    Arch5,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L271
/// Branch speculation outcome classification.
///
/// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/93315e46b000fc80fff5d53c3f444417fb3df6de>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BranchSpec {
    // PERF_BR_SPEC_NA
    /// Not available.
    Na,
    // PERF_BR_SPEC_WRONG_PATH
    /// Speculative but on wrong path.
    Wrong,
    // PERF_BR_SPEC_CORRECT_PATH
    /// Speculative and on correct path.
    Correct,
    // PERF_BR_NON_SPEC_CORRECT_PATH
    /// Non-speculative but on correct path.
    NoSpecCorrect,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L291
/// Branch privilege levels.
///
/// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/5402d25aa5710d240040f73fb13d7d5c303ef071>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BranchPriv {
    // PERF_BR_PRIV_UNKNOWN
    Unknown,
    // PERF_BR_PRIV_USER
    User,
    // PERF_BR_PRIV_KERNEL
    Kernel,
    // PERF_BR_PRIV_HV
    Hv,
}

/// Sampling weight.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Weight {
    Full(u64),
    Vars { var1: u32, var2: u16, var3: u16 },
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L322
/// The sources of any transactional memory aborts.
///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Txn {
    // PERF_TXN_ELISION
    /// From elision.
    pub elision: bool,
    // PERF_TXN_TRANSACTION
    /// From transaction.
    pub tx: bool,
    // PERF_TXN_SYNC
    /// Aborts caused by current thread.
    pub is_sync: bool,
    // PERF_TXN_ASYNC
    /// Aborts caused by other theads.
    pub is_async: bool,
    // PERF_TXN_RETRY
    /// Retryable transaction.
    pub retry: bool,
    // PERF_TXN_CONFLICT
    /// Conflicts with other threads.
    pub conflict: bool,
    // PERF_TXN_CAPACITY_READ
    /// Transaction write capacity overflow.
    pub capacity_read: bool,
    // PERF_TXN_CAPACITY_WRITE
    /// Transaction read capacity overflow.
    pub capacity_write: bool,
    // (flags & PERF_TXN_ABORT_MASK) >> PERF_TXN_ABORT_SHIFT
    /// User-specified abort code.
    pub code: u32,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1286
/// The source of data associated with the sampled instruction.
///
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DataSource {
    /// Type of opcode.
    pub op: MemOp,
    /// Memory hierarchy levels.
    pub level: MemLevel,
    /// Snoop mode.
    pub snoop: MemSnoop,
    /// Locked instruction.
    pub lock: MemLock,
    /// TLB access.
    pub tlb: MemTlb,
    /// Memory hierarchy levels (V2).
    ///
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/6ae5fa61d27dcb055f4198bcf6c8dbbf1bb33f52>
    pub level2: MemLevel2,
    /// This can be combined with the memory hierarchy levels to signify a remote cache.
    ///
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/6ae5fa61d27dcb055f4198bcf6c8dbbf1bb33f52>
    pub remote: bool,
    /// Access blocked.
    ///
    /// Since `linux-5.12`: <https://github.com/torvalds/linux/commit/61b985e3e775a3a75fda04ce7ef1b1aefc4758bc>
    pub block: MemBlock,
    /// Hop level.
    ///
    /// Since `linux-5.16`: <https://github.com/torvalds/linux/commit/fec9cc6175d0ec1e13efe12be491d9bd4de62f80>
    pub hops: MemHop,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1324
/// Type of opcode.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemOp {
    // PERF_MEM_OP_NA
    /// Not available.
    pub na: bool,
    // PERF_MEM_OP_LOAD
    /// Load instruction.
    pub load: bool,
    // PERF_MEM_OP_STORE
    /// Store instruction.
    pub store: bool,
    // PERF_MEM_OP_PFETCH
    /// Prefetch.
    pub prefetch: bool,
    // PERF_MEM_OP_EXEC
    /// Code execution.
    pub exec: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1338
/// Memory hierarchy levels.
///
/// This is being deprecated in favour of [`MemLevel2`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemLevel {
    // PERF_MEM_LVL_NA
    /// Not available.
    pub na: bool,
    // PERF_MEM_LVL_HIT
    /// Hit level.
    pub hit: bool,
    // PERF_MEM_LVL_MISS
    /// Miss level.
    pub miss: bool,
    // PERF_MEM_LVL_L1
    /// L1.
    pub l1: bool,
    // PERF_MEM_LVL_LFB
    /// Line fill buffer.
    pub lfb: bool,
    // PERF_MEM_LVL_L2
    /// L2.
    pub l2: bool,
    // PERF_MEM_LVL_L3
    /// L3.
    pub l3: bool,
    // PERF_MEM_LVL_LOC_RAM
    /// Local DRAM.
    pub loc_ram: bool,
    // PERF_MEM_LVL_REM_RAM1
    /// Remote DRAM (1 hop).
    pub rem_ram1: bool,
    // PERF_MEM_LVL_REM_RAM2
    /// Remote DRAM (2 hops).
    pub rem_ram2: bool,
    // PERF_MEM_LVL_REM_CCE1
    /// Remote cache (1 hop).
    pub rem_cce1: bool,
    // PERF_MEM_LVL_REM_CCE2
    /// Remote cache (2 hops).
    pub rem_cce2: bool,
    // PERF_MEM_LVL_IO
    /// I/O memory.
    pub io: bool,
    // PERF_MEM_LVL_UNC
    /// Uncached memory.
    pub unc: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1376
/// Snoop mode.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemSnoop {
    // PERF_MEM_SNOOP_NA
    /// Not available.
    pub na: bool,
    // PERF_MEM_SNOOP_NONE
    /// No snoop.
    pub none: bool,
    // PERF_MEM_SNOOP_HIT
    /// Snoop hit.
    pub hit: bool,
    // PERF_MEM_SNOOP_MISS
    /// Snoop miss.
    pub miss: bool,
    // PERF_MEM_SNOOP_HITM
    /// Snoop hit modified.
    pub hit_m: bool,
    // PERF_MEM_SNOOPX_FWD
    /// Forward.
    /// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/6ae5fa61d27dcb055f4198bcf6c8dbbf1bb33f52>
    pub fwd: bool,
    // PERF_MEM_SNOOPX_PEER
    /// Transfer from peer.
    ///
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/cfef80bad4cf79cdc964a53c98254dfa462be83f>
    ///
    /// NOTE: This feature was first available in the perf tool in Linux 6.0,
    /// so it seems we should enable it in feature `linux-6.0`:
    /// <https://github.com/torvalds/linux/commit/2e21bcf0514a3623b41962bf424dec061c02ebc6>
    pub peer: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1388
/// Locked instruction.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemLock {
    // PERF_MEM_LOCK_NA
    /// Not available.
    pub na: bool,
    // PERF_MEM_LOCK_LOCKED
    /// Locked transaction.
    pub locked: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1393
/// TLB access.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemTlb {
    // PERF_MEM_TLB_NA
    /// Not available.
    pub na: bool,
    // PERF_MEM_TLB_HIT
    /// Hit level.
    pub hit: bool,
    // PERF_MEM_TLB_MISS
    /// Miss level.
    pub miss: bool,
    // PERF_MEM_TLB_L1
    /// L1.
    pub l1: bool,
    // PERF_MEM_TLB_L2
    /// L2.
    pub l2: bool,
    // PERF_MEM_TLB_WK
    /// Hardware walker.
    pub walker: bool,
    // PERF_MEM_TLB_OS
    /// OS fault handler.
    pub fault: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1357
/// Memory hierarchy levels (V2).
///
/// Since `linux-4.14`: <https://github.com/torvalds/linux/commit/6ae5fa61d27dcb055f4198bcf6c8dbbf1bb33f52>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MemLevel2 {
    // PERF_MEM_LVLNUM_L1
    /// L1.
    L1,
    // PERF_MEM_LVLNUM_L2
    /// L2.
    L2,
    // PERF_MEM_LVLNUM_L3
    /// L3.
    L3,
    // PERF_MEM_LVLNUM_L4
    /// L4.
    L4,
    // PERF_MEM_LVLNUM_L2_MHB
    /// L2 miss handling buffer.
    ///
    /// Since `linux-6.11`: <https://github.com/torvalds/linux/commit/608f6976c309793ceea37292c54b057dab091944>
    L2Mhb,
    // PERF_MEM_LVLNUM_MSC
    /// Memory-side cache.
    ///
    /// Since `linux-6.11`: <https://github.com/torvalds/linux/commit/608f6976c309793ceea37292c54b057dab091944>
    Msc,
    // PERF_MEM_LVLNUM_UNC
    /// Uncached.
    ///
    /// Since `linux-6.6`: <https://github.com/torvalds/linux/commit/526fffabc5fb63e80eb890c74b6570df2570c87f>
    Unc,
    // PERF_MEM_LVLNUM_CXL
    /// CXL.
    ///
    /// Since `linux-6.1`:
    /// <https://github.com/torvalds/linux/commit/cb6c18b5a41622c7a439508f7421f8766a91cb87>
    /// <https://github.com/torvalds/linux/commit/ee3e88dfec23153d0675b5d00522297b9adf657c>
    Cxl,
    // PERF_MEM_LVLNUM_IO
    /// I/O.
    ///
    /// Since `linux-6.1`: <https://github.com/torvalds/linux/commit/ee3e88dfec23153d0675b5d00522297b9adf657c>
    Io,
    // PERF_MEM_LVLNUM_ANY_CACHE
    /// Any cache.
    AnyCache,
    // PERF_MEM_LVLNUM_LFB
    /// LFB / L1 Miss Handling Buffer.
    Lfb,
    // PERF_MEM_LVLNUM_RAM
    /// RAM.
    Ram,
    // PERF_MEM_LVLNUM_PMEM
    /// PMEM.
    Pmem,
    // PERF_MEM_LVLNUM_NA
    /// Not available.
    Na,
    /// Unknown.
    ///
    /// This is for compatibility, not ABI.
    Unknown,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1403
/// Access blocked.
///
/// Since `linux-5.12`: <https://github.com/torvalds/linux/commit/61b985e3e775a3a75fda04ce7ef1b1aefc4758bc>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemBlock {
    // PERF_MEM_BLK_NA
    /// Not available.
    pub na: bool,

    // PERF_MEM_BLK_DATA
    /// Data could not be forwarded.
    pub data: bool,

    // PERF_MEM_BLK_ADDR
    /// Address conflict.
    pub addr: bool,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1409
// https://github.com/torvalds/linux/blob/v6.13/tools/perf/util/mem-events.c#L385
/// Hop levels.
///
/// Since `linux-5.16`: <https://github.com/torvalds/linux/commit/fec9cc6175d0ec1e13efe12be491d9bd4de62f80>
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MemHop {
    // PERF_MEM_HOPS_0
    /// Remote core, same node.
    Core,
    // PERF_MEM_HOPS_1
    /// Remote node, same socket.
    ///
    /// Since `linux-5.17`: <https://github.com/torvalds/linux/commit/cb1c4aba055f928ffae0c868e8dfe08eeab302e7>
    Node,
    // PERF_MEM_HOPS_2
    /// Remote socket, same board.
    ///
    /// Since `linux-5.17`: <https://github.com/torvalds/linux/commit/cb1c4aba055f928ffae0c868e8dfe08eeab302e7>
    Socket,
    // PERF_MEM_HOPS_3
    /// Remote board.
    ///
    /// Since `linux-5.17`: <https://github.com/torvalds/linux/commit/cb1c4aba055f928ffae0c868e8dfe08eeab302e7>
    Board,
    /// Unknown.
    ///
    /// This is for compatibility, not ABI.
    Unknown,
}

/// Type of ABI.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Abi {
    /// 32-bit ABI.
    _32,
    /// 64-bit ABI.
    _64,
}
