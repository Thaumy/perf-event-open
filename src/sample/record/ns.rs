use super::{RecordId, Task};

/// Namespace information for the new task.
///
/// # Examples
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.extra_record.namespaces = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// // Captures the namespace information for this task.
/// std::thread::spawn(|| {});
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
/// }
/// ```
///
/// Since `linux-4.12`: <https://github.com/torvalds/linux/commit/e422267322cd319e2695a535e47c5b1feeac45eb>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Namespaces {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Task info.
    pub task: Task,
    // UTS namespace link info.
    pub ns_uts: LinkInfo,
    // PID namespace link info.
    pub ns_pid: LinkInfo,
    // IPC namespace link info.
    pub ns_ipc: LinkInfo,
    // Mount namespace link info.
    pub ns_mnt: LinkInfo,
    // Network namespace link info.
    pub ns_net: LinkInfo,
    // User namespace link info.
    pub ns_user: LinkInfo,
    // Cgroup namespace link info.
    pub ns_cgroup: LinkInfo,
}

impl Namespaces {
    #[cfg(feature = "linux-4.12")]
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

super::from!(Namespaces);

super::debug!(Namespaces {
    {record_id?},
    {task},
    {ns_net},
    {ns_uts},
    {ns_ipc},
    {ns_pid},
    {ns_user},
    {ns_mnt},
    {ns_cgroup},
});

// Naming: https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8590
/// Namespace link info.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinkInfo {
    /// Device number.
    pub dev: u64,

    /// Inode number.
    pub inode: u64,
}
