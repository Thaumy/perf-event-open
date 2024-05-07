use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::io::{self, Result};
use std::os::fd::AsRawFd;
use std::rc::Rc;
use std::sync::Arc;

use super::{Counter, Stat};
use crate::config::sibling::attr::from;
use crate::config::sibling::Opts;
use crate::event::Event;
use crate::ffi::bindings as b;
use crate::ffi::syscall::{ioctl_arg, perf_event_open};

pub struct CounterGroup {
    leader: Counter,

    // Keeps all siblings alive with the leader.
    //
    // We use `Rc` here because `CounterGroup` is not intended to be `Send`.
    //
    // There are three reasons:
    //
    // - A vector of `Arc<Counter>` does not let `CounterGroup` to be `Send`
    // because `Counter` is unsafe to be `Sync` for performance reasons.
    //
    // - A sendable `CounterGroup` could leave some references of sibling
    // counters (such as `Arc<Counter>`) using `add()` operation in one
    // thread, and get those refernces via `siblings()` in the other thread,
    // which potentially breaks the `!Sync` bound for `Counter`.
    //
    // - We could send `Counter` and consume it by `CounterGroup::from` to
    // avoid the `!Send` drawback of `CounterGroup`, so that's not a problem.
    siblings: Vec<Rc<Counter>>,
}

impl CounterGroup {
    pub fn from(leader: Counter) -> Self {
        Self {
            leader,
            siblings: vec![],
        }
    }

    pub fn leader(&self) -> &Counter {
        &self.leader
    }

    pub fn siblings(&self) -> &[Rc<Counter>] {
        self.siblings.as_slice()
    }

    pub fn add(
        &mut self,
        event: impl TryInto<Event, Error = io::Error>,
        opts: impl Borrow<Opts>,
    ) -> Result<Rc<Counter>> {
        let leader = &self.leader;

        let attr = {
            // We only change the attr fields related to event config,
            // which are not used to initialize the sibling attr.
            let leader_attr = unsafe { &*leader.attr.get() };
            from(event.try_into()?.0, opts.borrow(), leader_attr)?
        };
        let group_fd = leader.perf.as_raw_fd();
        // All events in a group should monitor the same task (or cgroup) and CPU:
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12932
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L992
        // https://github.com/torvalds/linux/blob/v6.13/kernel/events/core.c#L12926
        let flags = leader.target.flags | b::PERF_FLAG_FD_CLOEXEC as u64;
        let perf = perf_event_open(&attr, leader.target.pid, leader.target.cpu, group_fd, flags)?;

        let sibling = Rc::new(Counter {
            target: leader.target.clone(),
            attr: UnsafeCell::new(attr),
            perf: Arc::new(perf),
            read_buf: {
                let mut base = vec![];
                // `group::StatFormat` has no `PERF_FORMAT_GROUP` for sibling event,
                // so set `group_size` to 1 is safe.
                Stat::alloc_read_buf(&mut base, 1, attr.read_format);
                UnsafeCell::new(base)
            },
        });

        self.siblings.push(Rc::clone(&sibling));

        // Counter group and group leader always lives in the same thread,
        // there could be only up to one borrow to the `read_buf` at the same time.
        let base = unsafe { &mut *leader.read_buf.get() };
        // We only change the attr fields related to event config,
        // there is nothing about `read_format`.
        let leader_read_format = unsafe { &*leader.attr.get() }.read_format;
        Stat::alloc_read_buf(base, self.siblings.len() + 1, leader_read_format);

        Ok(sibling)
    }

    pub fn enable(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_ENABLE as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_DISABLE as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }

    pub fn clear_count(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_RESET as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }
}
