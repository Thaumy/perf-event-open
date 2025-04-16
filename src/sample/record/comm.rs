use std::ffi::{CStr, CString};

use super::{RecordId, SampleType, Task};
use crate::ffi::{bindings as b, deref_offset};

/// Process name (comm) has been changed.
///
/// # Examples
///
/// ```rust
/// use std::ffi::CStr;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
/// # use perf_event_open::sample::record::Record;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.extra_record.comm = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
///
/// // Change the process name to "foo" to trigger the `Comm` record.
/// let name = CStr::from_bytes_with_nul(b"foo\0").unwrap();
/// unsafe { libc::prctl(libc::PR_SET_NAME, name.as_ptr()) };
///
/// # let mut vec = vec![];
/// let mut iter = sampler.iter();
/// while let Some(it) = iter.next() {
///     println!("{:-?}", it);
///     # vec.push(it);
/// }
/// # assert!(vec.iter().any(|(_, it)| matches!(it, Record::Comm(_))));
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Comm {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Whether comm was changed by `execve`.
    pub by_execve: bool,
    /// Task info.
    pub task: Task,
    /// New comm.
    pub comm: CString,
}

impl Comm {
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        misc: u16,
        sample_id_all: Option<SampleType>,
    ) -> Self {
        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L901
        // struct {
        //     struct perf_event_header header;
        //     u32  pid, tid;
        //     char comm[];
        //     struct sample_id sample_id;
        // };

        let task = Task {
            pid: deref_offset(&mut ptr),
            tid: deref_offset(&mut ptr),
        };
        let comm = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(comm.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8540
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        let by_execve = misc & b::PERF_RECORD_MISC_COMM_EXEC as u16 > 0;

        Self {
            record_id,
            by_execve,
            task,
            comm,
        }
    }
}

super::from!(Comm);

super::debug!(Comm {
    {record_id?},
    {by_execve},
    {task},
    {comm},
});
