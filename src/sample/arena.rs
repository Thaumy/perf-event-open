use std::fs::File;
use std::io::Result;
use std::ptr::{null_mut, NonNull};
use std::slice;

use crate::ffi::syscall::{mmap, munmap};

pub struct Arena {
    ptr: NonNull<u8>,
    len: usize,
}

impl Arena {
    pub fn new(file: &File, len: usize, offset: usize) -> Result<Self> {
        let prot = libc::PROT_READ | libc::PROT_WRITE;
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L6582
        let flags = libc::MAP_SHARED;
        let ptr = unsafe { mmap(null_mut(), len, prot, flags, file, offset as _) }?.cast();
        Ok(Self { ptr, len })
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        match unsafe { munmap(self.ptr.as_ptr() as _, self.len) } {
            Ok(()) => (),
            Err(e) => panic!("Failed to unmap arena: {}", e),
        }
    }
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L580
// struct perf_event_mmap_page {
//     u32 version;        /* version number of this structure */
//     u32 compat_version; /* lowest version this is compat with */
//
//     u32 lock;         /* seqlock for synchronization */
//     u32 index;        /* hardware event identifier */
//     s64 offset;       /* add to hardware event value */
//     u64 time_enabled; /* time event active */
//     u64 time_running; /* time event on CPU */
//     union {
//         u64 capabilities;
//         struct {
//             u64 cap_bit0              : 1, /* Always 0, deprecated, see commit 860f085b74e9 */
//                 cap_bit0_is_deprecated: 1, /* Always 1, signals that bit 0 is zero */
//                 cap_user_rdpmc        : 1, /* The RDPMC instruction can be used to read counts */
//                 cap_user_time         : 1, /* The time_{shift,mult,offset} fields are used */
//                 cap_user_time_zero    : 1, /* The time_zero field is used */
//                 cap_user_time_short   : 1, /* the time_{cycle,mask} fields are used */
//                 cap_____res           : 58;
//         };
//     };
//
//     u16 pmc_width;
//
//     u16 time_shift;
//     u32 time_mult;
//     u64 time_offset;
//     u64 time_zero;
//
//     u32 size;
//     u32 __reserved_1;
//
//     u64 time_cycles;
//     u64 time_mask;
//
//     u8 __reserved[116*8];
//
//     u64 data_head;   /* head in the data section */
//     u64 data_tail;   /* user-space written tail */
//     u64 data_offset; /* where the buffer starts */
//     u64 data_size;   /* data buffer size */
//
//     u64 aux_head;
//     u64 aux_tail;
//     u64 aux_offset;
//     u64 aux_size;
// };
