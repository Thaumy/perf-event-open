use std::ffi::CString;

use super::RecordId;

/// Process created a new cgroup.
///
/// # Examples
///
/// Running this example may require root privileges.
///
/// ```rust, no_run
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::Dummy;
/// let target = (Proc::CURRENT, Cpu::ALL);
/// let mut opts = Opts::default();
/// opts.extra_record.cgroup = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(5).unwrap();
///
/// counter.enable().unwrap();
/// let path = format!("/sys/fs/cgroup/{}", uuid::Uuid::new_v4());
/// std::fs::create_dir(&path).unwrap();
/// std::fs::remove_dir(&path).unwrap();
///
/// # let mut cgroup_record = false;
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     # use perf_event_open::sample::record::Record;
///     # if let Record::Cgroup(c) = &it.1 {
///     #     use std::path::Path;
///     #     assert_eq!(
///     #         Path::new(&c.path.to_string_lossy().to_string())
///     #             .file_name()
///     #             .unwrap(),
///     #         Path::new(&path).file_name().unwrap()
///     #     );
///     #     cgroup_record = true;
///     # }
/// }
/// # assert!(cgroup_record);
/// ```
///
/// Since `linux-5.7`: <https://github.com/torvalds/linux/commit/96aaab686505c449e24d76e76507290dcc30e008>
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cgroup {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Cgroup ID.
    pub id: u64,
    /// Cgroup path from the root cgroup.
    pub path: CString,
}

impl Cgroup {
    #[cfg(feature = "linux-5.7")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::ffi::CStr;

        use super::SampleType;
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.13/include/uapi/linux/perf_event.h#L1187
        // struct {
        //     struct perf_event_header header;
        //     u64 id;
        //     char path[];
        //     struct sample_id sample_id;
        // };

        let id = deref_offset(&mut ptr);
        let path = CStr::from_ptr(ptr as _).to_owned();
        let record_id = sample_id_all.map(|SampleType(ty)| {
            ptr = ptr.add(path.as_bytes_with_nul().len());
            // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L8791
            ptr = ptr.add(ptr.align_offset(align_of::<u64>()));
            RecordId::from_ptr(ptr, ty)
        });

        Self {
            record_id,
            id,
            path,
        }
    }
}

super::from!(Cgroup);

super::debug!(Cgroup {
    {record_id?},
    {id},
    {path},
});
