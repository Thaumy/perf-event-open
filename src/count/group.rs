use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::io::{self, Result};
use std::mem;
use std::os::fd::AsRawFd;
use std::rc::Rc;
use std::sync::Arc;

use super::{Counter, Stat};
use crate::config::sibling::attr::from;
use crate::config::sibling::Opts;
use crate::event::Event;
use crate::ffi::bindings as b;
use crate::ffi::syscall::{ioctl_arg, perf_event_open};

/// Counter group.
///
/// An event group is scheduled onto the CPU as a unit: it will be put onto
/// the CPU only if all of the events in the group can be put onto the CPU.
///
/// This means that the values of the member events can be meaningfully compared, added,
/// divided (to get ratios), and so on with each other, since they have counted events
/// for the same set of executed instructions.
///
/// # Examples
///
/// ```rust
/// use std::thread;
/// use std::time::Duration;
///
/// use perf_event_open::config::{Cpu, Opts, Proc};
/// use perf_event_open::count::group::CounterGroup;
/// use perf_event_open::count::Counter;
/// use perf_event_open::event::hw::Hardware;
///
/// let target = (Proc::ALL, Cpu(0)); // All processes on CPU 0.
///
/// let mut opts = Opts::default();
/// opts.stat_format.siblings = true; // Collect sibling counts in leader stat.
///
/// let leader = Counter::new(Hardware::Instr, target, opts).unwrap();
///
/// let mut group = CounterGroup::from(leader);
/// group.add(Hardware::CpuCycle, &Default::default()).unwrap();
///
/// group.enable().unwrap();
/// thread::sleep(Duration::from_millis(100));
/// group.disable().unwrap();
///
/// let stat = group.leader().stat().unwrap();
/// let instrs = stat.count;
/// let cycles = stat.siblings[0].count;
///
/// println!("IPC: {}", instrs as f64 / cycles as f64);
/// ```
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
    /// Create group with leader counter.
    pub fn from(leader: Counter) -> Self {
        Self {
            leader,
            siblings: vec![],
        }
    }

    /// Returns a reference to the leader of the counter group.
    pub fn leader(&self) -> &Counter {
        &self.leader
    }

    /// Returns the sibling counters of the counter group in the order they were added.
    pub fn siblings(&self) -> &[Rc<Counter>] {
        self.siblings.as_slice()
    }

    /// Add sibling event to group.
    ///
    /// All siblings share the same [target][crate::config::Target] with the group leader.
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
        // `group::StatFormat` has no `PERF_FORMAT_GROUP` for sibling event,
        // so set `group_size` to 1 is safe.
        let read_buf = vec![0; Stat::read_buf_size(1, attr.read_format)];

        let sibling = Rc::new(Counter {
            target: leader.target.clone(),
            attr: UnsafeCell::new(attr),
            perf: Arc::new(perf),
            read_buf: UnsafeCell::new(read_buf),
        });

        self.siblings.push(Rc::clone(&sibling));

        // We only change the attr fields related to event config,
        // there is nothing about `read_format`.
        let leader_read_format = unsafe { &*leader.attr.get() }.read_format;
        let new_len = Stat::read_buf_size(self.siblings.len() + 1, leader_read_format);
        // Counter group and group leader always lives in the same thread,
        // there could be only up to one borrow to the `read_buf` at the same time.
        let old = unsafe { &mut *leader.read_buf.get() };
        if new_len > old.len() {
            // We allocate a new buffer instead of resizing the old one to avoid
            // the copying old data unnecessarily.
            //
            // Because `vec![0; n]` is optimized to use `calloc`, the real
            // allocation will happen in the `Counter::stat` call, so there
            // is no overhead in calling `add` multiple times.
            let new = vec![0; new_len];
            let _ = mem::replace(old, new);
        }

        Ok(sibling)
    }

    /// Enables all counters in the group.
    pub fn enable(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_ENABLE as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }

    /// Disables all counters in the group.
    pub fn disable(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_DISABLE as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }

    /// Clears the counts of all counters in the group.
    pub fn clear_count(&self) -> Result<()> {
        ioctl_arg(
            &self.leader.perf,
            b::PERF_IOC_OP_RESET as _,
            b::PERF_IOC_FLAG_GROUP as _,
        )?;
        Ok(())
    }
}
