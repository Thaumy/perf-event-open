use super::{RecordId, SampleType, Task};
use crate::ffi::deref_offset;

/// Process exited.
///
/// Please check module-level docs for examples.
///
/// # Examples
///
/// ```rust
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::mpsc::channel;
/// use std::thread;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
/// # use perf_event_open::sample::record::Record;
///
/// static WAIT: AtomicBool = AtomicBool::new(true);
///
/// let (tid_tx, tid_rx) = channel();
/// let handle = thread::spawn(move || {
///     tid_tx.send(unsafe { libc::gettid() }).unwrap();
///     while WAIT.load(Ordering::Relaxed) {
///         std::hint::spin_loop();
///     }
///     thread::spawn(|| {}); // Fork here.
/// });
///
/// let event = Software::Dummy;
/// let target = (Proc(tid_rx.recv().unwrap() as _), Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.extra_record.task = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
/// WAIT.store(false, Ordering::Relaxed);
/// handle.join().unwrap(); // Exit here.
///
/// # let mut vec = vec![];
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     # vec.push(it);
/// }
/// # assert!(vec.iter().any(|(_, it)| matches!(it, Record::Fork(_))));
/// # assert!(vec.iter().any(|(_, it)| matches!(it, Record::Exit(_))));
/// ```
///
/// See also [`ExtraRecords::task`][crate::config::ExtraRecord::task].
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Exit {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Task info.
    pub task: Task,
    /// Parent task info.
    pub parent_task: Task,
    /// Timestamp.
    pub time: u64,
}

impl Exit {
    pub(crate) unsafe fn from_ptr(mut ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L912
        // struct {
        //     struct perf_event_header header;
        //     u32 pid, ppid;
        //     u32 tid, ptid;
        //     u64 time;
        //     struct sample_id sample_id;
        // };

        let pid = deref_offset(&mut ptr);
        let ppid = deref_offset(&mut ptr);
        let tid = deref_offset(&mut ptr);
        let ptid = deref_offset(&mut ptr);

        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8428
        let time = deref_offset(&mut ptr);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        let task = Task { pid, tid };
        let parent_task = Task {
            pid: ppid,
            tid: ptid,
        };

        Self {
            record_id,
            task,
            parent_task,
            time,
        }
    }
}

super::from!(Exit);

super::debug!(Exit {
    {record_id?},
    {task},
    {parent_task},
    {time},
});

/// Process forked.
///
/// See [`Exit`] for examples.
///
/// See also [`ExtraRecords::task`][crate::config::ExtraRecord::task].
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fork {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Task info.
    pub task: Task,
    /// Parent task info.
    pub parent_task: Task,
    /// Timestamp.
    pub time: u64,
}

impl Fork {
    pub(crate) unsafe fn from_ptr(ptr: *const u8, sample_id_all: Option<SampleType>) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8423
        let layout = Exit::from_ptr(ptr, sample_id_all);

        Self {
            record_id: layout.record_id,
            task: layout.task,
            parent_task: layout.parent_task,
            time: layout.time,
        }
    }
}

super::from!(Fork);

super::debug!(Fork {
    {record_id?},
    {task},
    {parent_task},
    {time},
});
