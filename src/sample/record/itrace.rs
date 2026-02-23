use super::{RecordId, Task};

/// Instruction tracing has started.
///
/// Instruction tracing is a hardware feature that identifies every
/// branch taken by a program so that we can reconstruct the actual
/// control flow of the program.
///
/// Only limited platforms support this feature, such as Intel PT, Intel BTS, and Arm SPE.
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
/// Since `linux-4.1`: <https://github.com/torvalds/linux/commit/ec0d7729bbaed4b9d2d3fada693278e13a3d1368>
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItraceStart {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Task info.
    pub task: Task,
}

impl ItraceStart {
    #[cfg(feature = "linux-4.1")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1093
        // struct {
        //     struct perf_event_header header;
        //     u32 pid;
        //     u32 tid;
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let record_id = sample_id_all.map(|super::SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self { record_id, task }
    }
}

super::from!(ItraceStart);

super::debug!(ItraceStart {
    {record_id?},
    {task},
});
