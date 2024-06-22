use std::ffi::CString;

use super::RecordId;

/// Since `linux-5.1`: <https://github.com/torvalds/linux/commit/76193a94522f1d4edf2447a536f3f796ce56343b>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ksymbol {
    pub record_id: Option<RecordId>,

    pub ty: Type,
    pub name: CString,
    pub state: State,
    pub addr: u64,
    pub len: u32,
}

impl Ksymbol {
    #[cfg(feature = "linux-5.1")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::ffi::CStr;
        use std::mem::align_of;

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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum State {
    Reg,
    Unreg,
}

// https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1232
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Type {
    // PERF_RECORD_KSYMBOL_TYPE_BPF
    Bpf,
    // PERF_RECORD_KSYMBOL_TYPE_OOL
    /// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/69e49088692899d25dedfa22f00dfb9761e86ed7>
    OutOfLine,
    // PERF_RECORD_KSYMBOL_TYPE_UNKNOWN
    Unknown,
}
