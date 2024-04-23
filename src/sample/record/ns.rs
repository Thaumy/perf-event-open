use super::{RecordId, Task};

#[derive(Clone)]
pub struct Namespaces {
    pub record_id: Option<RecordId>,

    pub task: Task,
    pub ns_uts: LinkInfo,
    pub ns_pid: LinkInfo,
    pub ns_ipc: LinkInfo,
    pub ns_mnt: LinkInfo,
    pub ns_net: LinkInfo,
    pub ns_user: LinkInfo,
    pub ns_cgroup: LinkInfo,
}

impl Namespaces {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use super::SampleType;
        use crate::ffi::{bindings as b, deref_offset};

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1141
        // struct {
        //     struct perf_event_header header;
        //     u32 pid;
        //     u32 tid;
        //     u64 nr_namespaces;
        //     { u64 dev, inode; } [nr_namespaces];
        //     struct sample_id sample_id;
        // }

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct Layout {
            dev: u64,
            inode: u64,
        }
        impl From<Layout> for LinkInfo {
            fn from(value: Layout) -> Self {
                Self {
                    dev: value.dev,
                    inode: value.inode,
                }
            }
        }
        let nss: [Layout; b::NR_NAMESPACES as _] = deref_offset(&mut ptr);

        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            task,
            ns_net: nss[b::NET_NS_INDEX as usize].into(),
            ns_uts: nss[b::UTS_NS_INDEX as usize].into(),
            ns_ipc: nss[b::IPC_NS_INDEX as usize].into(),
            ns_pid: nss[b::PID_NS_INDEX as usize].into(),
            ns_user: nss[b::USER_NS_INDEX as usize].into(),
            ns_mnt: nss[b::MNT_NS_INDEX as usize].into(),
            ns_cgroup: nss[b::CGROUP_NS_INDEX as usize].into(),
        }
    }
}

// Naming: https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8590
#[derive(Clone, Debug)]
pub struct LinkInfo {
    pub dev: u64,
    pub inode: u64,
}
