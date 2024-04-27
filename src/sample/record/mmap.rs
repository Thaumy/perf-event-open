use std::ffi::{CStr, CString};
use std::mem::align_of;

use arrayvec::ArrayVec;

use super::{RecordId, SampleType, Task};
use crate::ffi::{bindings as b, deref_offset};

// https://github.com/torvalds/linux/blob/v6.13/include/linux/buildid.h#L7
const BUILD_ID_SIZE_MAX: usize = 20;

#[derive(Clone)]
pub struct Mmap {
    pub record_id: Option<RecordId>,

    pub executable: bool,
    pub task: Task,
    pub addr: u64,
    pub len: u64,
    pub file: CString,
    pub page_offset: u64,
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
    // extends the output if `attr.mmap2` was enabled
    // Call chain: `perf_event_mmap` -> `perf_event_mmap_event` -> `perf_event_mmap_output`
    // `perf_event_mmap` set `type` to `PERF_RECORD_MMAP`:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9129
    // `perf_event_mmap_event` set `misc` to `PERF_RECORD_MISC_MMAP_DATA` if the map is inexecutable:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9004
    // `perf_event_mmap_output` overwrite `type` to `PERF_RECORD_MMAP2` if `attr.mmap2` was enabled:
    // https://github.com/torvalds/linux/blob/v6.12/kernel/events/core.c#L8815
    // `perf_event_mmap_output` extends the output:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8884
    // So the final output ABI is:
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

#[derive(Clone, Debug)]
pub struct Ext {
    pub prot: u32,
    pub flags: u32,
    pub info: Info,
}

#[derive(Clone, Debug)]
pub enum Info {
    Device {
        major: u32,
        minor: u32,
        inode: u64,
        inode_gen: u64,
    },
    BuildId(ArrayVec<u8, BUILD_ID_SIZE_MAX>),
}
