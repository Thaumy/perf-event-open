use std::ffi::{CStr, CString};

use arrayvec::ArrayVec;

use super::{RecordId, SampleType, Task};
use crate::ffi::{bindings as b, deref_offset};

// https://github.com/torvalds/linux/blob/v6.13/include/linux/buildid.h#L7
const BUILD_ID_SIZE_MAX: usize = 20;

/// Process memory-mapped.
///
/// This is useful if we want to correlate user-space IPs to code.
///
/// # Examples
///
/// ```rust
/// use std::ptr::null_mut;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
/// # use perf_event_open::sample::record::Record;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.extra_record.mmap.code = true; // Capture executable mmaps.
/// opts.extra_record.mmap.data = true; // Capture non-executable mmaps.
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// let flags = libc::MAP_ANONYMOUS | libc::MAP_SHARED;
/// let len = 4096;
/// unsafe {
///     libc::mmap(null_mut(), len, libc::PROT_EXEC, flags, -1, 0);
///     libc::mmap(null_mut(), len, libc::PROT_READ, flags, -1, 0);
/// };
///
/// # let mut count = 0;
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     # if matches!(it.1, Record::Mmap(_)) {
///     #     count += 1;
///     # }
/// }
/// # assert_eq!(count, 2);
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mmap {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Executable mapping.
    pub executable: bool,
    /// Task info.
    pub task: Task,
    /// Address.
    pub addr: u64,
    /// Length.
    pub len: u64,
    /// Mapped file.
    pub file: CString,
    /// Page offset.
    pub page_offset: u64,
    /// Extension fields.
    pub ext: Option<Ext>,
}

impl Mmap {
    // PERF_RECORD_MMAP
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L877
    // struct {
    //     struct perf_event_header header;
    //     u32 pid, tid;
    //     u64 addr;
    //     u64 len;
    //     u64 pgoff;
    //     char filename[];
    //     struct sample_id sample_id;
    // };
    //
    // PERF_RECORD_MMAP2
    // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1048
    // struct {
    //     struct perf_event_header header;
    //     u32 pid, tid;
    //     u64 addr;
    //     u64 len;
    //     u64 pgoff;
    //     union {
    //         struct {
    //             u32 maj;
    //             u32 min;
    //             u64 ino;
    //             u64 ino_generation;
    //         };
    //         struct {
    //             u8 build_id_size;
    //             u8 __reserved_1;
    //             u16 __reserved_2;
    //             u8 build_id[20];
    //         };
    //     };
    //     u32 prot, flags;
    //     char filename[];
    //     struct sample_id sample_id;
    // };
    //
    // `PERF_RECORD_MMAP` and `PERF_RECORD_MMAP2` shares the same output and will never appear together
    // in the same ring-buffer since kernel replaces `PERF_RECORD_MMAP` with `PERF_RECORD_MMAP2` and
    // extends the output if `attr.mmap2` was enabled.
    //
    // Call chain: `perf_event_mmap` -> `perf_event_mmap_event` -> `perf_event_mmap_output`
    // `perf_event_mmap` set `type` to `PERF_RECORD_MMAP`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9129
    // `perf_event_mmap_event` set `misc` to `PERF_RECORD_MISC_MMAP_DATA` if the map is inexecutable:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9004
    // `perf_event_mmap_output` overwrite `type` to `PERF_RECORD_MMAP2` if `attr.mmap2` was enabled:
    // https://github.com/torvalds/linux/blob/v6.12/kernel/events/core.c#L8815
    // `perf_event_mmap_output` extends the output:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8884
    //
    // So the final output ABI would be:
    // struct {
    //     struct perf_event_header header;
    //     u32 pid, tid;
    //     u64 addr;
    //     u64 len;
    //     u64 pgoff;
    //     {
    //         union {
    //             struct {
    //                 u32 maj;
    //                 u32 min;
    //                 u64 ino;
    //                 u64 ino_generation;
    //             };
    //             struct {
    //                 u8 build_id_size;
    //                 u8 __reserved_1;
    //                 u16 __reserved_2;
    //                 u8 build_id[20];
    //             };
    //         };
    //     } && PERF_RECORD_MMAP2
    //     { u32 prot, flags; } && PERF_RECORD_MMAP2
    //     char filename[];
    //     struct sample_id sample_id;
    // };
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        misc: u16,
        v2: bool,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let addr = deref_offset(&mut ptr);
        let len = deref_offset(&mut ptr);
        let page_offset = deref_offset(&mut ptr);

        let ext = v2.then(|| {
            #[cfg(feature = "linux-5.12")]
            let info = if misc as u32 & b::PERF_RECORD_MISC_MMAP_BUILD_ID > 0 {
                let len = deref_offset::<u8>(&mut ptr) as usize;
                ptr = ptr.add(3); // Skip reserved bits.
                let build_id = {
                    let slice = std::slice::from_raw_parts(ptr, len);
                    let result = ArrayVec::try_from(slice);
                    // len <= BUILD_ID_SIZE_MAX
                    unsafe { result.unwrap_unchecked() }
                };
                ptr = ptr.add(BUILD_ID_SIZE_MAX);
                Info::BuildId(build_id)
            } else {
                Info::Device {
                    major: deref_offset(&mut ptr),
                    minor: deref_offset(&mut ptr),
                    inode: deref_offset(&mut ptr),
                    inode_gen: deref_offset(&mut ptr),
                }
            };
            #[cfg(not(feature = "linux-5.12"))]
            let info = Info::Device {
                major: deref_offset(&mut ptr),
                minor: deref_offset(&mut ptr),
                inode: deref_offset(&mut ptr),
                inode_gen: deref_offset(&mut ptr),
            };
            let prot = deref_offset(&mut ptr);
            let flags = deref_offset(&mut ptr);
            Ext { prot, flags, info }
        });

        let file = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(file.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8992
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        let executable = misc as u32 & b::PERF_RECORD_MISC_MMAP_DATA == 0;

        Self {
            record_id,
            executable,
            task,
            addr,
            len,
            file,
            page_offset,
            ext,
        }
    }
}

super::from!(Mmap);

super::debug!(Mmap {
    {record_id?},
    {executable},
    {task},
    {addr},
    {len},
    {file},
    {page_offset},
    {ext?},
});

/// Extension fields.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ext {
    /// Protection info.
    pub prot: u32,
    /// Flags info.
    pub flags: u32,
    /// Device info or ELF file build ID.
    pub info: Info,
}

/// Device info or ELF file build ID.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Info {
    /// Device info.
    Device {
        /// Major number.
        major: u32,
        /// Minor number.
        minor: u32,
        /// Inode number.
        inode: u64,
        /// Inode generation.
        inode_gen: u64,
    },
    /// ELF file build ID.
    ///
    /// See also [`UseBuildId`][crate::config::UseBuildId].
    ///
    /// Since `linux-5.12`: <https://github.com/torvalds/linux/commit/88a16a1309333e43d328621ece3e9fa37027e8eb>
    BuildId(ArrayVec<u8, BUILD_ID_SIZE_MAX>),
}
