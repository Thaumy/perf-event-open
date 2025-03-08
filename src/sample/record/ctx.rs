use super::{RecordId, Task};

/// Process context switched.
///
/// # Examples
///
/// ```rust
/// # #[cfg(not(feature = "linux-4.3"))]
/// # return;
/// #
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
/// # use perf_event_open::sample::record::ctx::Switch;
/// # use perf_event_open::sample::record::Record;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.extra_record.ctx_switch = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// // Triggers a context switch.
/// thread::sleep(Duration::from_millis(1));
///
/// # let mut switch_out = false;
/// # let mut switch_in = false;
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     # if let Record::CtxSwitch(it) = it.1 {
///     #     match it.info {
///     #         Switch::OutTo { .. } => switch_out = true,
///     #         Switch::InFrom(_) => switch_in = true,
///     #     }
///     # }
/// }
/// # assert!(switch_out);
/// # assert!(switch_in);
/// ```
///
/// Since `linux-4.3`: <https://github.com/torvalds/linux/commit/45ac1403f564f411c6a383a2448688ba8dd705a4>
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CtxSwitch {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Context switch info.
    pub info: Switch,
}

impl CtxSwitch {
    #[cfg(feature = "linux-4.3")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        cpu_wide: bool,
        misc: u16,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use super::SampleType;
        use crate::ffi::{bindings as b, deref_offset};

        // PERF_RECORD_SWITCH
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1119
        // struct {
        //     struct perf_event_header header;
        //     struct sample_id sample_id;
        // };
        //
        // PERF_RECORD_SWITCH_CPU_WIDE
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1131
        // struct {
        //     struct perf_event_header header;
        //     u32 next_prev_pid;
        //     u32 next_prev_tid;
        //     struct sample_id sample_id;
        // };

        let task = cpu_wide.then(|| Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        });
        let info = if misc as u32 & b::PERF_RECORD_MISC_SWITCH_OUT > 0 {
            #[cfg(feature = "linux-4.17")]
            let preempt = misc as u32 & b::PERF_RECORD_MISC_SWITCH_OUT_PREEMPT > 0;
            #[cfg(not(feature = "linux-4.17"))]
            let preempt = false;
            Switch::OutTo { task, preempt }
        } else {
            Switch::InFrom(task)
        };
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self { record_id, info }
    }
}

super::from!(CtxSwitch);

super::debug!(CtxSwitch {
    {record_id?},
    {info},
});

// Some(task) if PERF_RECORD_SWITCH_CPU_WIDE
/// Context switch info.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Switch {
    // PERF_RECORD_MISC_SWITCH_OUT
    /// Switched out.
    OutTo {
        /// Info of the task being switched to.
        task: Option<Task>,
        // PERF_RECORD_MISC_SWITCH_OUT_PREEMPT
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9298
        // https://github.com/torvalds/linux/blob/v6.13/tools/perf/util/scripting-engines/trace-event-python.c#L1571
        /// Indicates whether the context switch was triggered by preemption.
        ///
        /// Since `linux-4.17`: <https://github.com/torvalds/linux/commit/101592b4904ecf6b8ed2a4784d41d180319d95a1>
        preempt: bool,
    },
    // !PERF_RECORD_MISC_SWITCH_OUT
    /// Switched in.
    ///
    /// Contains info of the task being switched from if possible.
    InFrom(Option<Task>),
}
