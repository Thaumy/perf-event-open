use std::ffi::CString;

use super::RecordId;

/// Kernel symbol event.
///
/// # Examples
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
/// opts.extra_record.ksymbol = true;
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
/// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/76193a94522f1d4edf2447a536f3f796ce56343b>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ksymbol {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Type.
    pub ty: Type,
    /// Name.
    pub name: CString,
    /// State.
    pub state: State,
    /// Address.
    pub addr: u64,
    /// Length.
    pub len: u32,
}

impl Ksymbol {
    #[cfg(feature = "linux-5.1")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::ffi::CStr;

        use super::SampleType;
        use crate::ffi::{bindings as b, deref_offset};

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1155
        // struct {
        //     struct perf_event_header header;
        //     u64 addr;
        //     u32 len;
        //     u16 ksym_type;
        //     u16 flags;
        //     char name[];
        //     struct sample_id sample_id;
        // };

        let addr = deref_offset(&mut ptr);
        let len = deref_offset(&mut ptr);
        let ty = match deref_offset::<u16>(&mut ptr) as _ {
            b::PERF_RECORD_KSYMBOL_TYPE_BPF => Type::Bpf,
            #[cfg(feature = "linux-5.9")]
            b::PERF_RECORD_KSYMBOL_TYPE_OOL => Type::OutOfLine,
            b::PERF_RECORD_KSYMBOL_TYPE_UNKNOWN => Type::Unknown,
            _ => Type::Unknown, // For compatibility, not ABI.
        };
        let flags: u16 = deref_offset(&mut ptr);
        let name = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(name.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9409
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9413
        let state = if flags as u32 & b::PERF_RECORD_KSYMBOL_FLAGS_UNREGISTER > 0 {
            State::Reg
        } else {
            State::Unreg
        };

        Ksymbol {
            record_id,
            ty,
            name,
            state,
            addr,
            len,
        }
    }
}

super::from!(Ksymbol);

super::debug!(Ksymbol {
    {record_id?},
    {ty},
    {name},
    {state},
    {addr},
    {len},
});

/// Ksymbol state.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum State {
    /// Register.
    Reg,
    /// Unregister.
    Unreg,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1232
/// Ksymbol type.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    // PERF_RECORD_KSYMBOL_TYPE_BPF
    /// BPF program.
    Bpf,
    // PERF_RECORD_KSYMBOL_TYPE_OOL
    /// Out of line code such as kprobe-replaced instructions or optimized kprobes.
    ///
    /// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/69e49088692899d25dedfa22f00dfb9761e86ed7>
    OutOfLine,
    // PERF_RECORD_KSYMBOL_TYPE_UNKNOWN
    /// Unknown.
    Unknown,
}
