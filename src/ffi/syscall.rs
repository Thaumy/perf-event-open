use std::fs::File;
use std::io::{Error, Result};
use std::mem::{transmute, MaybeUninit};
use std::os::fd::{AsRawFd, FromRawFd};

use libc::epoll_event;

use super::Attr;

pub fn perf_event_open(attr: &Attr, pid: i32, cpu: i32, group_fd: i32, flags: u64) -> Result<File> {
    let num = libc::SYS_perf_event_open;
    let fd = unsafe { libc::syscall(num, attr, pid, cpu, group_fd, flags) };
    if fd != -1 {
        Ok(unsafe { File::from_raw_fd(fd as _) })
    } else {
        Err(Error::last_os_error())
    }
}

pub fn ioctl(file: &File, op: u64) -> Result<i32> {
    let fd = file.as_raw_fd();
    let result = unsafe { libc::ioctl(fd, op as _) };
    if result != -1 {
        Ok(result)
    } else {
        Err(Error::last_os_error())
    }
}

pub fn ioctl_arg(file: &File, op: u64, arg: u64) -> Result<i32> {
    let fd = file.as_raw_fd();
    let result = unsafe { libc::ioctl(fd, op as _, arg) };
    if result != -1 {
        Ok(result)
    } else {
        Err(Error::last_os_error())
    }
}

pub fn ioctl_argp<T: ?Sized>(file: &File, op: u64, argp: &mut T) -> Result<i32> {
    let fd = file.as_raw_fd();
    let result = unsafe { libc::ioctl(fd, op as _, argp) };
    if result != -1 {
        Ok(result)
    } else {
        Err(Error::last_os_error())
    }
}

pub fn read(file: &File, buf: &mut [u8]) -> Result<usize> {
    let fd = file.as_raw_fd();
    let count = buf.len();
    let buf = buf.as_mut_ptr() as _;
    let bytes = unsafe { libc::read(fd, buf, count) };
    if bytes != -1 {
        Ok(bytes as _)
    } else {
        Err(Error::last_os_error())
    }
}

pub fn read_uninit(file: &File, buf: &mut [MaybeUninit<u8>]) -> Result<usize> {
    let buf = unsafe { transmute::<&mut [_], &mut [u8]>(buf) };
    read(file, buf)
}

pub unsafe fn mmap<T>(
    ptr: *mut (),
    len: usize,
    prot: i32,
    flags: i32,
    file: &File,
    offset: i64,
) -> Result<*mut T> {
    let ptr = libc::mmap(ptr as _, len, prot, flags, file.as_raw_fd(), offset);
    if ptr != libc::MAP_FAILED {
        Ok(ptr as _)
    } else {
        Err(Error::last_os_error())
    }
}

pub unsafe fn munmap<T>(ptr: *mut T, len: usize) -> Result<()> {
    let result = libc::munmap(ptr as _, len);
    if result != -1 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

pub fn epoll_create1(flags: i32) -> Result<File> {
    let fd = unsafe { libc::epoll_create1(flags) };
    if fd != -1 {
        Ok(unsafe { File::from_raw_fd(fd as _) })
    } else {
        Err(Error::last_os_error())
    }
}

pub fn epoll_ctl(epoll: &File, op: i32, file: &File, event: &mut epoll_event) -> Result<()> {
    let result = unsafe { libc::epoll_ctl(epoll.as_raw_fd(), op, file.as_raw_fd(), event as _) };
    if result != -1 {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}

pub fn epoll_wait<'a>(
    epoll: &File,
    events: &'a mut [epoll_event],
    timeout: i32,
) -> Result<&'a [epoll_event]> {
    let len = unsafe {
        libc::epoll_wait(
            epoll.as_raw_fd(),
            events.as_mut_ptr(),
            events.len() as _,
            timeout,
        )
    };
    if len != -1 {
        Ok(&events[..len as _])
    } else {
        Err(Error::last_os_error())
    }
}
