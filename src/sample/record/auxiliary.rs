use super::RecordId;

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aux {
    pub record_id: Option<RecordId>,

    pub offset: u64,
    pub size: u64,

    // PERF_AUX_FLAG_TRUNCATED
    pub truncated: bool,
    // PERF_AUX_FLAG_OVERWRITE
    pub overwrite: bool,
    // PERF_AUX_FLAG_PARTIAL
    pub partial: bool,
    // PERF_AUX_FLAG_COLLISION
    pub collision: bool,
    // `flags` masked with `PERF_AUX_FLAG_PMU_FORMAT_TYPE_MASK`
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/547b60988e631f74ed025cf1ec50cfc17f49fd13>
    pub pmu_format_type: u8,
}

impl Aux {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use crate::ffi::{bindings as b, deref_offset};

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1079
        // struct {
        //     struct perf_event_header header;
        //     u64 aux_offset;
        //     u64 aux_size;
        //     u64 flags;
        //     struct sample_id sample_id;
        // };

        let offset = deref_offset(&mut ptr);
        let size = deref_offset(&mut ptr);

        let flags = deref_offset::<u64>(&mut ptr);
        macro_rules! when {
            ($($feature: literal,)? $flag:ident) => {{
                $(#[cfg(feature = $feature)])?
                let val = flags & b::$flag as u64 > 0;
                $(
                #[cfg(not(feature = $feature))]
                let val = false;
                )?
                val
            }};
        }
        let truncated = when!(PERF_AUX_FLAG_TRUNCATED);
        let overwrite = when!(PERF_AUX_FLAG_OVERWRITE);
        let partial = when!(PERF_AUX_FLAG_PARTIAL);
        let collision = when!(PERF_AUX_FLAG_COLLISION);
        #[cfg(feature = "linux-5.13")]
        let pmu_format_type = {
            let masked = flags & b::PERF_AUX_FLAG_PMU_FORMAT_TYPE_MASK as u64;
            (masked >> 8) as _
        };
        #[cfg(not(feature = "linux-5.13"))]
        let pmu_format_type = 0;

        let record_id = sample_id_all.map(|super::SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            offset,
            size,
            truncated,
            overwrite,
            partial,
            collision,
            pmu_format_type,
        }
    }
}

super::from!(Aux);

super::debug!(Aux {
    {record_id?},
    {offset},
    {size},
    {truncated},
    {overwrite},
    {partial},
    {collision},
    {pmu_format_type},
});

/// Since `linux-5.16`: <https://github.com/torvalds/linux/commit/8b8ff8cc3b8155c18162e8b1f70e1230db176862>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuxOutputHwId {
    pub record_id: Option<RecordId>,

    pub hw_id: u64,
}

impl AuxOutputHwId {
    #[cfg(feature = "linux-5.16")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1221
        // struct {
        //     struct perf_event_header header;
        //     u64 hw_id;
        //     struct sample_id sample_id;
        // };

        let hw_id = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|super::SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self { record_id, hw_id }
    }
}

super::from!(AuxOutputHwId);

super::debug!(AuxOutputHwId {
    {record_id?},
    {hw_id},
});
