use super::{RecordId, SampleType, Task};
use crate::count::Stat;
use crate::ffi::deref_offset;

/// Inherited task statistics.
///
/// This allows a per-task stat on an inherited process hierarchy.
///
/// NOTE: This record can be genreated by enabling `inherit` and `remove_on_exec`
/// if there is an execve call in the target process. But triggering it by exiting
/// task seems broken, we may need to debug the kernel implementation to find out
/// why, so there is no example for this record now. This situation can also be
/// reproduced by `perf record -s` and `perf report -T` commands, which share the
/// same perf attr as our test case.
#[derive(Clone)]
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
