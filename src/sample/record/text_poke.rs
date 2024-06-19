use super::RecordId;

/// Since `linux-5.9`: <https://github.com/torvalds/linux/commit/e17d43b93e544f5016c0251d2074c15568d5d963>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextPoke {
    pub record_id: Option<RecordId>,

    pub addr: u64,
    pub old_bytes: Vec<u8>,
    pub new_bytes: Vec<u8>,
}

impl TextPoke {
    #[cfg(feature = "linux-5.9")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::mem::align_of;
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
