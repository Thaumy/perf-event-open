use std::io::Result;

use super::{Opts, SampleOn, UseBuildId, WakeUpOn};
use crate::config::{Inherit, OnExecve, Repr};
use crate::event::EventConfig;
use crate::ffi::{bindings as b, Attr};

pub(crate) fn from(event_cfg: EventConfig, opts: &Opts) -> Result<Attr> {
    let mut attr = Attr {
        size: size_of::<Attr>() as _,
        ..Default::default()
    };

    // event config:

    attr.type_ = event_cfg.ty;
    attr.config = event_cfg.config;
    attr.__bindgen_anon_3.config1 = event_cfg.config1;
    attr.__bindgen_anon_4.config2 = event_cfg.config2;
    #[cfg(feature = "linux-6.3")]
    (attr.config3 = event_cfg.config3);
    #[cfg(not(feature = "linux-6.3"))]
    crate::config::unsupported!(event_cfg.config3 > 0);
    attr.bp_type = event_cfg.bp_type;

    // count config:

    macro_rules! then {
        ($then:tt) => {
            attr.$then(1)
        };
    }
    macro_rules! when {
        ($bool:ident, $then:tt) => {
            if opts.exclude.$bool {
                then!($then);
            }
        };
    }
    when!(user, set_exclude_user);
    when!(kernel, set_exclude_kernel);
    when!(hv, set_exclude_hv);
    when!(host, set_exclude_host);
    when!(guest, set_exclude_guest);
    when!(idle, set_exclude_idle);

    attr.set_exclusive(opts.only_group as _);
    attr.set_pinned(opts.pin_on_pmu as _);

    match opts.inherit {
        Some(Inherit::NewChild) => {
            then!(set_inherit);
        }
        #[cfg(feature = "linux-5.13")]
        Some(Inherit::NewThread) => {
            then!(set_inherit);
            then!(set_inherit_thread);
        }
        #[cfg(not(feature = "linux-5.13"))]
        Some(Inherit::NewThread) => crate::config::unsupported!(),
        None => (),
    }

    match opts.on_execve {
        Some(OnExecve::Enable) => then!(set_enable_on_exec),
        #[cfg(feature = "linux-5.13")]
        Some(OnExecve::Remove) => then!(set_remove_on_exec),
        #[cfg(not(feature = "linux-5.13"))]
        Some(OnExecve::Remove) => crate::config::unsupported!(),
        None => (),
    }

    attr.read_format = opts.stat_format.as_read_format()?;
    attr.set_disabled(!opts.enable as _);

    // sample config:

    match opts.sample_on {
        SampleOn::Freq(val) => {
            then!(set_freq);
            attr.__bindgen_anon_1.sample_freq = val;
        }
        SampleOn::Count(val) => {
            attr.__bindgen_anon_1.sample_period = val;
        }
    }

    attr.set_precise_ip(opts.sample_skid.as_precise_ip() as _);

    // The internal variant `__PERF_SAMPLE_CALLCHAIN_EARLY`(Linux 4.18-6.0) widens
    // other enum variants from u32 to u64, so we need to convert the veriant to u64
    // before assigning it to `attr.sample_type`. We liverage the type system here to
    // infer the type of `sample_type` and then assign it to `attr.sample_type` to
    // avoid clippy's noise about unnecessary cast from `linux-4.18` to `linux-6.0`.
    // For more information about `__PERF_SAMPLE_CALLCHAIN_EARLY`, see:
    // https://github.com/torvalds/linux/commit/6cbc304f2f360f25cc8607817239d6f4a2fd3dc5
    // https://github.com/torvalds/linux/commit/b4e12b2d70fd9eccdb3cef8015dc1788ca38e3fd
    let mut sample_type = 0;
    macro_rules! when {
        ($($feature:literal,)? $bool:ident, $flag:ident) => {
            if opts.sample_format.$bool {
                $(#[cfg(feature = $feature)])?
                (sample_type |= b::$flag);
                $(
                #[cfg(not(feature = $feature))]
                crate::config::unsupported!(opts.sample_format.$bool);
                )?
            }
        };
        ($($feature:literal,)? $option:ident, $it:ident, $effect:tt) => {
            $(#[cfg(feature = $feature)])?
            if let Some($it) = opts.sample_format.$option.as_ref() {
                $effect;
            }
            $(
            #[cfg(not(feature = $feature))]
            crate::config::unsupported!(opts.sample_format.$option.is_some());
            )?
        };
    }
    when!(stat, PERF_SAMPLE_READ);
    when!(period, PERF_SAMPLE_PERIOD);
    when!(cgroup, PERF_SAMPLE_CGROUP);
    when!(user_stack, it, {
        attr.sample_stack_user = it.0;
        sample_type |= b::PERF_SAMPLE_STACK_USER;
    });
    when!(call_chain, it, {
        attr.set_exclude_callchain_user(it.exclude_user as _);
        attr.set_exclude_callchain_kernel(it.exclude_kernel as _);
        attr.sample_max_stack = it.max_stack_frames;
        sample_type |= b::PERF_SAMPLE_CALLCHAIN;
    });
    when!(data_addr, PERF_SAMPLE_ADDR);
    when!(data_phys_addr, PERF_SAMPLE_PHYS_ADDR);
    when!("linux-5.11", data_page_size, PERF_SAMPLE_DATA_PAGE_SIZE);
    when!(data_source, PERF_SAMPLE_DATA_SRC);
    when!(code_addr, PERF_SAMPLE_IP);
    when!("linux-5.11", code_page_size, PERF_SAMPLE_CODE_PAGE_SIZE);
    when!(user_regs, it, {
        attr.sample_regs_user = it.0;
        sample_type |= b::PERF_SAMPLE_REGS_USER;
    });
    when!(intr_regs, it, {
        attr.sample_regs_intr = it.0;
        sample_type |= b::PERF_SAMPLE_REGS_INTR;
    });
    when!(raw, PERF_SAMPLE_RAW);
    when!(lbr, it, {
        attr.branch_sample_type = it
            .target_priv
            .as_ref()
            .map(|it| it.as_branch_sample_type())
            .unwrap_or_default();

        macro_rules! when {
            ($($feature:literal,)? $bool:ident, $flag:ident) => {
                if it.branch_type.$bool {
                    $(#[cfg(feature = $feature)])?
                    (attr.branch_sample_type |= b::$flag as u64);
                    $(
                    #[cfg(not(feature = $feature))]
                    crate::config::unsupported!();
                    )?
                }
            };
        }
        when!(any, PERF_SAMPLE_BRANCH_ANY);
        when!(any_return, PERF_SAMPLE_BRANCH_ANY_RETURN);
        when!(cond, PERF_SAMPLE_BRANCH_COND);
        when!(ind_jump, PERF_SAMPLE_BRANCH_IND_JUMP);
        when!(call_stack, PERF_SAMPLE_BRANCH_CALL_STACK);
        when!(call, PERF_SAMPLE_BRANCH_CALL);
        when!(any_call, PERF_SAMPLE_BRANCH_ANY_CALL);
        when!(ind_call, PERF_SAMPLE_BRANCH_IND_CALL);
        when!(in_tx, PERF_SAMPLE_BRANCH_IN_TX);
        when!(no_tx, PERF_SAMPLE_BRANCH_NO_TX);
        when!(abort_tx, PERF_SAMPLE_BRANCH_ABORT_TX);

        if it.hw_index {
            attr.branch_sample_type |= b::PERF_SAMPLE_BRANCH_HW_INDEX as u64;
        }

        macro_rules! when {
            ($($feature:literal,)? $bool:ident, $flag:ident) => {
                if it.entry_format.$bool {
                    $(#[cfg(feature = $feature)])?
                    (attr.branch_sample_type |= b::$flag as u64);
                    $(
                    #[cfg(not(feature = $feature))]
                    crate::config::unsupported!();
                    )?
                }
            };
        }
        if !it.entry_format.flags {
            attr.branch_sample_type |= b::PERF_SAMPLE_BRANCH_NO_FLAGS as u64;
        }
        if !it.entry_format.cycles {
            attr.branch_sample_type |= b::PERF_SAMPLE_BRANCH_NO_CYCLES as u64;
        }
        when!("linux-6.8", counter, PERF_SAMPLE_BRANCH_COUNTERS);
        when!(branch_type, PERF_SAMPLE_BRANCH_TYPE_SAVE);
        when!("linux-6.1", branch_priv, PERF_SAMPLE_BRANCH_PRIV_SAVE);

        sample_type |= b::PERF_SAMPLE_BRANCH_STACK;
    });
    when!(aux, it, {
        attr.aux_sample_size = it.0;
        sample_type |= b::PERF_SAMPLE_AUX;
    });
    when!(txn, PERF_SAMPLE_TRANSACTION);
    when!(weight, it, {
        sample_type |= match it {
            Repr::Full => b::PERF_SAMPLE_WEIGHT,
            #[cfg(feature = "linux-5.12")]
            Repr::Vars => b::PERF_SAMPLE_WEIGHT_STRUCT,
            #[cfg(not(feature = "linux-5.12"))]
            Repr::Vars => crate::config::unsupported!(),
        };
    });
    macro_rules! when {
        ($bool:ident, $flag:ident) => {
            if opts.record_id_format.$bool {
                sample_type |= b::$flag;
            }
        };
    }
    when!(id, PERF_SAMPLE_ID);
    when!(stream_id, PERF_SAMPLE_STREAM_ID);
    when!(cpu, PERF_SAMPLE_CPU);
    when!(task, PERF_SAMPLE_TID);
    when!(time, PERF_SAMPLE_TIME);
    attr.sample_type = sample_type as _;

    macro_rules! when {
        ($($feature:literal,)? $bool:ident, $then:tt) => {
            if opts.extra_record.$bool {
                $(#[cfg(feature = $feature)])?
                attr.$then(1);
                $(
                #[cfg(not(feature = $feature))]
                crate::config::unsupported!();
                )?
            }
        };
    }
    when!(task, set_task);
    when!(read, set_inherit_stat);
    when!(comm, set_comm);
    let mmap = &opts.extra_record.mmap;
    mmap.code.then(|| then!(set_mmap));
    mmap.data.then(|| then!(set_mmap_data));
    if let Some(UseBuildId(b)) = &mmap.ext {
        then!(set_mmap);
        then!(set_mmap2);
        #[cfg(feature = "linux-5.12")]
        attr.set_build_id(*b as _);
        #[cfg(not(feature = "linux-5.12"))]
        crate::config::unsupported!(*b);
    }
    when!(cgroup, set_cgroup);
    when!(ksymbol, set_ksymbol);
    when!(bpf_event, set_bpf_event);
    when!("linux-5.9", text_poke, set_text_poke);
    when!(ctx_switch, set_context_switch);
    when!(namespaces, set_namespaces);

    attr.set_sample_id_all(opts.record_id_all as _);

    match opts.wake_up.on {
        WakeUpOn::Bytes(n) => {
            then!(set_watermark);
            attr.__bindgen_anon_2.wakeup_watermark = n as _;
        }
        WakeUpOn::Samples(n) => {
            attr.__bindgen_anon_2.wakeup_events = n as _;
        }
    }

    #[cfg(feature = "linux-5.13")]
    if let Some(crate::config::SigData(data)) = opts.sigtrap_on_sample.as_ref() {
        then!(set_sigtrap);
        attr.sig_data = *data;
    }
    #[cfg(not(feature = "linux-5.13"))]
    crate::config::unsupported!(opts.sigtrap_on_sample.is_some());

    // AUX wakeup shares the same epoll with the normal wakeup, see:
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L556
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/ring_buffer.c#L24
    // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L6927
    attr.aux_watermark = opts.wake_up.on_aux_bytes;

    if let Some(clock) = opts.timer.as_ref() {
        then!(set_use_clockid);
        use crate::config::Clock;
        attr.clockid = match clock {
            Clock::Tai => b::CLOCK_TAI,
            Clock::RealTime => b::CLOCK_REALTIME,
            Clock::BootTime => b::CLOCK_BOOTTIME,
            Clock::Monotonic => b::CLOCK_MONOTONIC,
            Clock::MonotonicRaw => b::CLOCK_MONOTONIC_RAW,
        } as _;
    }

    #[cfg(feature = "linux-6.13")]
    {
        let aux_action = unsafe { &mut attr.__bindgen_anon_5.__bindgen_anon_1 };
        aux_action.set_aux_start_paused(opts.pause_aux as _);
    }
    #[cfg(not(feature = "linux-6.13"))]
    crate::config::unsupported!(opts.pause_aux);

    Ok(attr)
}
