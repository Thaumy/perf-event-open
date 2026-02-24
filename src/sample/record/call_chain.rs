use super::RecordId;

/// Deferred call chain.
///
/// Needs to be stitched to the previous incomplete call chain
/// to form the full one.
///
/// Can be enabled by [`ExtraRecord::call_chain_deferred`][crate::config::ExtraRecord::call_chain_deferred].
///
/// # Examples
///
/// ```rust
/// # #[cfg(not(feature = "linux-6.19"))]
/// # return;
/// #
/// use perf_event_open::config::{CallChain, Cpu, Opts, Proc, SampleOn};
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::sw::Software;
///
/// let event = Software::TaskClock;
/// let target = (Proc::CURRENT, Cpu::ALL);
///
/// let mut opts = Opts::default();
/// opts.sample_on = SampleOn::Count(1_000_000); // 1ms
/// opts.sample_format.call_chain = Some(CallChain {
///     exclude_user: false,
///     exclude_kernel: false,
///     // Request `CallChain::UserDeferred`.
///     defer_user: true,
///     max_stack_frames: 20,
/// });
/// // Generate `CallChainDeferred` record.
/// opts.extra_record.call_chain_deferred = true;
///
/// let counter = Counter::new(event, target, opts).unwrap();
/// let sampler = counter.sampler(10).unwrap();
///
/// counter.enable().unwrap();
/// // Make some noise to collect call chains.
/// for _ in 0..100000 {
///     unsafe { libc::gettid() };
/// }
/// counter.disable().unwrap();
/// #
/// # let mut user_deferred = false;
/// # let mut call_chain_deferred = false;
///
/// for it in sampler.iter() {
///     println!("{:-?}", it);
///     #
///     # use perf_event_open::sample::record::Record;
///     # use perf_event_open::sample::record::sample::CallChain;
///     # match it.1 {
///     #     Record::Sample(s) => {
///     #         if s.call_chain
///     #             .unwrap()
///     #             .iter()
///     #             .any(|it| matches!(it, CallChain::UserDeferred { cookie: _ }))
///     #         {
///     #             user_deferred = true;
///     #         }
///     #     }
///     #     Record::CallChainDeferred(_) => {
///     #         call_chain_deferred = true;
///     #     }
///     #     _ => {}
///     # }
/// }
/// #
/// # assert!(user_deferred);
/// # assert!(call_chain_deferred);
/// ```
///
/// See also [`CallChain::defer_user`][crate::config::CallChain::defer_user]
/// and [`CallChain::UserDeferred`][crate::sample::record::sample::CallChain::UserDeferred].
///
/// Since `linux-6.19`: <https://github.com/torvalds/linux/commit/c69993ecdd4dfde2b7da08b022052a33b203da07>
#[derive(Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallChainDeferred {
    /// Record IDs.
    pub record_id: Option<RecordId>,

    /// Cookie used to match the previous incomplete call chain.
    ///
    /// See also [`CallChain::UserDeferred`][crate::sample::record::sample::CallChain::UserDeferred].
    pub cookie: u64,
    /// Call chain in user context.
    pub call_chain: Vec<u64>,
}

impl CallChainDeferred {
    #[cfg(feature = "linux-6.19")]
    pub(crate) unsafe fn from_ptr(
        mut ptr: *const u8,
        sample_id_all: Option<super::SampleType>,
    ) -> Self {
        use std::slice;

        use super::SampleType;
        use crate::ffi::deref_offset;

        // https://github.com/torvalds/linux/blob/v6.19/include/uapi/linux/perf_event.h#L1252
        // struct {
        //     struct perf_event_header header;
        //     u64 cookie;
        //     u64 nr;
        //     u64 ips[nr];
        //     struct sample_id sample_id;
        // };

        let cookie = deref_offset(&mut ptr);

        let len = deref_offset::<u64>(&mut ptr) as usize;
        let call_chain = slice::from_raw_parts(ptr as *const u64, len).to_vec();
        ptr = ptr.add(len * size_of::<u64>());

        let record_id = sample_id_all.map(|SampleType(ty)| RecordId::from_ptr(ptr, ty));

        Self {
            record_id,
            cookie,
            call_chain,
        }
    }
}

super::from!(CallChainDeferred);

super::debug!(CallChainDeferred {
    {record_id?},
    {cookie},
    {call_chain},
});
