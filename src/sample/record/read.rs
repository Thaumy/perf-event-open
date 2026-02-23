use super::{RecordId, SampleType, Task};
use crate::count::Stat;
use crate::ffi::deref_offset;

/// Inherited task statistics.
///
/// This allows a per-task stat on an inherited process hierarchy.
///
/// # Examples
///
/// ```rust
/// use std::mem::MaybeUninit;
///
/// use perf_event_open::config::{Cpu, Inherit, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::TaskClock;
/// let target = (Proc::CURRENT, Cpu(0));
///
/// let mut opts = Opts::default();
/// opts.inherit = Some(Inherit::NewChild);
/// opts.extra_record.read = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// unsafe {
///     let child = libc::fork();
///     if child > 0 {
///         let mut code = 0;
///         libc::waitpid(child, &mut code as _, 0);
///         assert_eq!(code, 0);
///     } else {
///         // schedule child processes on CPU 0
///         let mut set = MaybeUninit::zeroed().assume_init();
///         libc::CPU_SET(0, &mut set);
///         let tid = libc::gettid();
///         let set_size = size_of_val(&set);
///         assert_eq!(libc::sched_setaffinity(tid, set_size, &set as _), 0);
///
///         // make some noise in the child process to kill time
///         for i in 0..100 {
///             std::hint::black_box(&i);
///         }
///         return;
///     }
/// }
///
/// let mut count = 0;
/// for it in sampler.iter() {
///     count += 1;
///     println!("{:-?}", it);
/// }
/// assert_eq!(count, 1);
/// ```
///
/// A kernel bug introduced in Linux 5.13 caused this feature to be unavailable;
/// this bug has been fixed in Linux 6.19. Therefore, you may not receive this
/// record if your Linux kernel does not include the fix, see [patch](https://github.com/torvalds/linux/commit/c418d8b4d7a43a86b82ee39cb52ece3034383530).
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Read {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Task info.
    pub task: Task,
    /// Counter statistics from the inherited task.
    pub stat: Stat,
}

impl Read {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        read_format: u64,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L946
        // struct {
        //     struct perf_event_header header;
        //     u32 pid, tid;
        //     struct read_format values;
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let stat = Stat::from_ptr_offset(&mut ptr, read_format);
        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            task,
            stat,
        }
    }
}

super::from!(Read);

super::debug!(Read {
    {record_id?},
    {task},
    {stat},
});
