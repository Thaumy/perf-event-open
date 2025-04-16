use super::RecordId;

/// Records changes to kernel text i.e. self-modified code.
///
/// # Examples
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use std::fs::File;
/// use std::os::fd::AsRawFd;
///
/// use perf_event_open::config::{Cpu, Opts, Proc, WakeUpOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.wake_up.on = WakeUpOn::Bytes(1);
/// opts.extra_record.ksymbol = true;
/// opts.extra_record.text_poke = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
/// counter.enable().unwrap();
///
/// // Insert a kernel module that registers a kprobe.
/// let file = File::open("register_kprobe_module.ko").unwrap();
/// unsafe {
///     libc::syscall(libc::SYS_finit_module, file.as_raw_fd(), b"\0", 0);
///     libc::syscall(libc::SYS_delete_module, b"register_kprobe_module\0", 0);
/// }
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
/// }
/// ```
///
/// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/e17d43b93e544f5016c0251d2074c15568d5d963>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextPoke {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Address.
    pub addr: u64,
    /// Old bytes.
    pub old_bytes: Vec<u8>,
    /// New bytes.
    pub new_bytes: Vec<u8>,
}

impl TextPoke {
    #[cfg(feature = "linux-5.9")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::slice;

        use super::SampleType;
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1203
        // struct {
        //     struct perf_event_header header;
        //     u64 addr;
        //     u16 old_len;
        //     u16 new_len;
        //     u8 bytes[];
        //     struct sample_id sample_id;
        // };

        let addr = deref_offset(&mut ptr);
        let old_len = deref_offset::<u16>(&mut ptr) as usize;
        let new_len = deref_offset::<u16>(&mut ptr) as usize;
        let bytes = slice::from_raw_parts(ptr, old_len + new_len);
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(bytes.len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9604
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        let old_bytes = bytes[..old_len].to_vec();
        let new_bytes = bytes[old_len..].to_vec();

        Self {
            record_id,
            addr,
            old_bytes,
            new_bytes,
        }
    }
}

super::from!(TextPoke);

super::debug!(TextPoke {
    {record_id?},
    {addr},
    {old_bytes},
    {new_bytes},
});
