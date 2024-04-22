use super::{RecordId, Task};

#[derive(Clone)]
pub struct CtxSwitch {
    pub record_id: Option<RecordId>,

    pub info: Switch,
}

impl CtxSwitch {
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
            let preempt = misc as u32 & b::PERF_RECORD_MISC_SWITCH_OUT_PREEMPT > 0;
            Switch::OutTo { task, preempt }
        } else {
            Switch::InFrom(task)
        };
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self { record_id, info }
    }
}

// Some(task) if PERF_RECORD_SWITCH_CPU_WIDE
#[derive(Clone, Debug)]
pub enum Switch {
    // PERF_RECORD_MISC_SWITCH_OUT
    OutTo {
        task: Option<Task>,
        // PERF_RECORD_MISC_SWITCH_OUT_PREEMPT
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L9298
        // https://github.com/torvalds/linux/blob/v6.13/tools/perf/util/scripting-engines/trace-event-python.c#L1571
        preempt: bool,
    },
    // !PERF_RECORD_MISC_SWITCH_OUT
    InFrom(Option<Task>),
}
