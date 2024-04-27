use super::RecordId;

const BPF_TAG_SIZE: u32 = crate::ffi::bindings::BPF_TAG_SIZE;

#[derive(Clone)]
pub struct BpfEvent {
    pub record_id: Option<RecordId>,

    pub ty: Type,
    pub id: u32,
    pub tag: [u8; BPF_TAG_SIZE as _],
    pub flags: u16,
}

impl BpfEvent {
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
#[derive(Clone, Debug)]
pub enum Type {
    // PERF_BPF_EVENT_PROG_LOAD
    ProgLoad,
    // PERF_BPF_EVENT_PROG_UNLOAD
    ProgUnload,
    // PERF_BPF_EVENT_UNKNOWN
    Unknown,
}
