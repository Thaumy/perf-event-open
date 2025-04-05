use super::RecordId;

/// New data is available in the AUX area.
///
/// # Examples
///
/// ```rust
/// use std::fs::read_to_string;
/// use std::sync::mpsc::channel;
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::dp::DynamicPmu;
///
/// let (tid_tx, tid_rx) = channel();
/// thread::spawn(move || {
///     tid_tx.send(unsafe { libc::gettid() }).unwrap();
///     loop {
///         std::hint::spin_loop();
///     }
/// });
///
/// // Intel PT
/// let ty = read_to_string("/sys/bus/event_source/devices/intel_pt/type");
/// # if ty.is_err() {
/// #     return;
/// # }
///
/// let event = DynamicPmu {
///     ty: ty.unwrap().lines().next().unwrap().parse().unwrap(),
///     config: 0,
///     config1: 0,
///     config2: 0,
///     config3: 0,
/// };
/// let target = (Proc(tid_rx.recv().unwrap() as _), Cpu::ALL);
/// let opts = Opts::default();
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(10).unwrap();
/// let aux = sampler.aux_tracer(10).unwrap();
///
/// counter.enable().unwrap();
/// thread::sleep(Duration::from_millis(1));
/// counter.disable().unwrap();
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
/// }
/// while let Some(it) = aux.iter().next(None) {
///     let bytes = it.len();
///     println!("{:.2} KB", bytes as f64 / 1000.0);
/// }
/// ```
///
/// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/68db7e98c3a6ebe7284b6cf14906ed7c55f3f7f0>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aux {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Data offset in the AUX area.
    pub offset: u64,
    /// Data size.
    pub size: u64,

    // PERF_AUX_FLAG_TRUNCATED
    /// Record was truncated to fit.
    pub truncated: bool,
    // PERF_AUX_FLAG_OVERWRITE
    /// Snapshot from overwrite mode.
    pub overwrite: bool,
    // PERF_AUX_FLAG_PARTIAL
    /// Record contains gaps.
    ///
    /// Since `linux-4.12`: <https://github.com/torvalds/linux/commit/ae0c2d995d648d5165545d5e05e2869642009b38>
    pub partial: bool,
    // PERF_AUX_FLAG_COLLISION
    /// Sample collided with another.
    ///
    /// Since `linux-4.15`: <https://github.com/torvalds/linux/commit/085b30625e39df67d7320f22269796276c6b0c11>
    pub collision: bool,
    // `flags` masked with `PERF_AUX_FLAG_PMU_FORMAT_TYPE_MASK`
    /// PMU specific trace format type.
    ///
    /// Since `linux-5.13`: <https://github.com/torvalds/linux/commit/547b60988e631f74ed025cf1ec50cfc17f49fd13>
    pub pmu_format_type: u8,
}

impl Aux {
    #[cfg(feature = "linux-4.1")]
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
        let partial = when!("linux-4.12", PERF_AUX_FLAG_PARTIAL);
        let collision = when!("linux-4.15", PERF_AUX_FLAG_COLLISION);
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

/// Hardware ID of the AUX output event.
///
/// Data written to the AUX area by hardware due to [`aux_output`][crate::config::sibling::Opts::aux_output],
/// may need to be matched to the event by an architecture-specific hardware ID.
/// This records the hardware ID, but requires [`RecordId`] to provide the
/// event ID. e.g. Intel PT uses this record to disambiguate PEBS-via-PT
/// records from multiple events.
///
/// Since `linux-5.16`: <https://github.com/torvalds/linux/commit/8b8ff8cc3b8155c18162e8b1f70e1230db176862>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AuxOutputHwId {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Hardware ID.
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
