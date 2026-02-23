use super::RecordId;

#[cfg(feature = "linux-5.1")]
const BPF_TAG_SIZE: u32 = crate::ffi::bindings::BPF_TAG_SIZE;
// NOTE: There is no `BPF_TAG_SIZE` before Linux 5.1, if the tag size changes
// in the future we need to ensure ABI compatibility.
#[cfg(not(feature = "linux-5.1"))]
const BPF_TAG_SIZE: u32 = 8;

/// BPF event.
///
/// # Examples
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// # tokio_test::block_on(async {
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::mpsc::channel;
/// use std::thread;
///
/// use perf_event_open::config::{Cpu, Opts, Proc, WakeUpOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// static WAIT: AtomicBool = AtomicBool::new(true);
///
/// let (tid_tx, tid_rx) = channel();
/// thread::spawn(move || {
///     tid_tx.send(unsafe { libc::gettid() }).unwrap();
///
///     while WAIT.load(Ordering::Relaxed) {
///         std::hint::spin_loop();
///     }
///
///     // Load a BPF program to trigger a `BpfEvent` record.
///     aya::Ebpf::load_file("HelloWorld.bpf.o").unwrap();
/// });
///
/// let event = Software::Dummy;
/// let target = (Proc(tid_rx.recv().unwrap() as _), Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.wake_up.on = WakeUpOn::Bytes(1);
/// opts.extra_record.bpf_event = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
/// WAIT.store(false, Ordering::Relaxed);
///
/// let mut iter = sampler.iter().into_async().unwrap();
/// while let Some(it) = iter.next().await {
///     println!("{:-?}", it);
/// }
/// # });
/// ```
///
/// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/6ee52e2a3fe4ea35520720736e6791df1fb67106>
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BpfEvent {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// BPF event type.
    pub ty: Type,
    /// BPF program ID.
    pub id: u32,
    /// BPF program tag.
    pub tag: [u8; BPF_TAG_SIZE as _],
    /// Flags.
    pub flags: u16,
}

impl BpfEvent {
    #[cfg(feature = "linux-5.1")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use super::SampleType;
        use crate::ffi::{bindings as b, deref_offset};

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1175
        // struct {
        //     struct perf_event_header header;
        //     u16 type;
        //     u16 flags;
        //     u32 id;
        //     u8 tag[BPF_TAG_SIZE];
        //     struct sample_id sample_id;
        // };

        let ty = match deref_offset::<u16>(&mut ptr) as _ {
            b::PERF_BPF_EVENT_PROG_LOAD => Type::ProgLoad,
            b::PERF_BPF_EVENT_PROG_UNLOAD => Type::ProgUnload,
            b::PERF_BPF_EVENT_UNKNOWN => Type::Unknown,
            _ => Type::Unknown, // For compatibility, not ABI.
        };
        let flags = deref_offset(&mut ptr);
        let id = deref_offset(&mut ptr);
        let tag = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            ty,
            id,
            tag,
            flags,
        }
    }
}

super::from!(BpfEvent);

super::debug!(BpfEvent {
    {record_id?},
    {ty},
    {id},
    {tag},
    {flags},
});

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1245
/// BPF event type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    // PERF_BPF_EVENT_PROG_LOAD
    /// BPF program load.
    ProgLoad,
    // PERF_BPF_EVENT_PROG_UNLOAD
    /// BPF program unload.
    ProgUnload,
    // PERF_BPF_EVENT_UNKNOWN
    /// Unknown.
    Unknown,
}
