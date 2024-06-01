use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::ffi::CStr;
use std::fs::File;
use std::io::{self, Result};
use std::mem::{transmute, MaybeUninit};
use std::os::fd::AsRawFd;
use std::sync::Arc;

use super::sample::Sampler;
use crate::config::attr::from;
use crate::config::{Opts, Target};
use crate::event::Event;
use crate::ffi::syscall::{ioctl, ioctl_arg, ioctl_argp, perf_event_open, read_uninit};
use crate::ffi::{bindings as b, Attr};

pub mod group;
mod stat;

pub use stat::*;

pub struct Counter {
    pub(crate) target: Target,
    pub(crate) attr: UnsafeCell<Attr>,
    pub(crate) perf: Arc<File>,
    pub(crate) read_buf: UnsafeCell<Vec<MaybeUninit<u8>>>,
}

impl Counter {
    pub fn new(
        event: impl TryInto<Event, Error = io::Error>,
        target: impl Into<Target>,
        opts: impl Borrow<Opts>,
    ) -> Result<Self> {
        let target = target.into();
        let attr = from(event.try_into()?.0, opts.borrow())?;
        let flags = target.flags | b::PERF_FLAG_FD_CLOEXEC as u64;
        let perf = perf_event_open(&attr, target.pid, target.cpu, -1, flags)?;

        Ok(Self {
            target,
            attr: UnsafeCell::new(attr),
            perf: Arc::new(perf),
            read_buf: {
                let mut base = vec![];
                // Now there is only one event in the group, if in the future
                // this counter becomes the leader, `CounterGroup::add_memnber`
                // will extend this buffer to sufficient size.
                Stat::alloc_read_buf(&mut base, 1, attr.read_format);
                UnsafeCell::new(base)
            },
        })
    }

    pub fn sampler(&self, exp: u8) -> Result<Sampler> {
        Sampler::new(self, exp)
    }

    pub fn file(&self) -> &File {
        &self.perf
    }

    pub fn id(&self) -> Result<u64> {
        let mut id = 0;
        ioctl_argp(&self.perf, b::PERF_IOC_OP_ID as _, &mut id)?;
        Ok(id)
    }

    pub fn enable(&self) -> Result<()> {
        ioctl(&self.perf, b::PERF_IOC_OP_ENABLE as _)?;
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        ioctl(&self.perf, b::PERF_IOC_OP_DISABLE as _)?;
        Ok(())
    }

    pub fn clear_count(&self) -> Result<()> {
        ioctl(&self.perf, b::PERF_IOC_OP_RESET as _)?;
        Ok(())
    }

    pub fn stat(&self) -> Result<Stat> {
        // There could be only up to one reference to `read_buf` at the same time,
        // since `Counter` is not `Sync`.
        let buf = unsafe { &mut *self.read_buf.get() };

        read_uninit(&self.perf, buf)?;
        let buf = buf.as_mut_slice();
        let buf = unsafe { transmute::<&mut [_], &mut [u8]>(buf) };

        let ptr = buf.as_ptr();
        // We only change the attr fields related to event config,
        // there is nothing about `read_format`.
        let read_format = unsafe { &*self.attr.get() }.read_format;
        let stat = unsafe { Stat::from_ptr(ptr, read_format) };

        Ok(stat)
    }

    pub fn attach_bpf(&self, file: &File) -> Result<()> {
        ioctl_arg(
            &self.perf,
            b::PERF_IOC_OP_SET_BPF as _,
            file.as_raw_fd() as _,
        )?;
        Ok(())
    }

    pub fn query_bpf(&self, buf_len: u32) -> Result<(Vec<u32>, Option<u32>)> {
        // struct perf_event_query_bpf {
        //     u32 ids_len;
        //     u32 prog_cnt;
        //     u32 ids[0];
        // }
        let mut buf = vec![MaybeUninit::uninit(); (2 + buf_len) as _];
        buf[0] = MaybeUninit::new(buf_len); // set `ids_len`

        match ioctl_argp(
            &self.perf,
            b::PERF_IOC_OP_QUERY_BPF as _,
            buf.as_mut_slice(),
        ) {
            Ok(_) => {
                let prog_cnt = unsafe { buf[1].assume_init() };

                let ids = buf[2..2 + (prog_cnt as usize)].to_vec();
                let ids = unsafe { transmute::<Vec<_>, Vec<u32>>(ids) };

                Ok((ids, None))
            }
            Err(e) => {
                let option = e.raw_os_error();

                // `option` is always `Some` since `Error` is constructed
                // by `ioctl_argp` via `Error::last_os_error`.
                let errno = unsafe { option.unwrap_unchecked() };

                if errno == libc::ENOSPC {
                    let prog_cnt = unsafe { buf[1].assume_init() };

                    let ids = buf[2..].to_vec();
                    let ids = unsafe { transmute::<Vec<_>, Vec<u32>>(ids) };

                    return Ok((ids, Some(prog_cnt - buf_len)));
                }

                Err(e)
            }
        }
    }

    pub fn with_ftrace_filter(&self, filter: &CStr) -> Result<()> {
        let ptr = filter.as_ptr() as *mut i8;

        // The following ioctl op just copies the bytes to kernel space,
        // so we don't have to worry about the mutable reference.
        let argp = unsafe { &mut *ptr };

        ioctl_argp(&self.perf, b::PERF_IOC_OP_RESET as _, argp)?;
        Ok(())
    }

    pub fn switch_to<E>(&self, event: E) -> Result<()>
    where
        E: TryInto<Event, Error = io::Error>,
    {
        let Event(event_cfg): Event = event.try_into()?;

        // We can only access `self.attr` within the same thread,
        // so there is no potential data race.
        //
        // We will only change fields about event config, this will
        // not break any consumptions or states since these fields
        // are never used elsewhere after the counter is initialized.
        //
        // The following ioctl op just copies the modified attr to kernel space,
        // so we don't have to worry about the mutable reference.
        let attr = unsafe { &mut *self.attr.get() };
        attr.type_ = event_cfg.ty;
        attr.config = event_cfg.config;
        attr.__bindgen_anon_3.config1 = event_cfg.config1;
        attr.__bindgen_anon_4.config2 = event_cfg.config2;
        #[cfg(feature = "linux-6.3")]
        (attr.config3 = event_cfg.config3);
        attr.bp_type = event_cfg.bp_type;

        ioctl_argp(&self.perf, b::PERF_IOC_OP_MODIFY_ATTRS as _, attr)?;

        Ok(())
    }
}
