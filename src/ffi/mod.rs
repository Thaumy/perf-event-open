use std::sync::LazyLock;

pub mod bindings;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod linux_syscall;

macro_rules! syscall {
    ($syscall:ident, $($arg:expr),* $(,)?) => {{
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let val = $crate::ffi::linux_syscall::$syscall($($arg),*);
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let val = {
            $(let _ = $arg;)*
            Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
        };
        val
    }};
}
pub(crate) use syscall;

// Dereferences the pointer and offsets by the size of the
// pointee type, then returns the dereferenced value.
#[inline]
pub unsafe fn deref_offset<T: Copy>(ptr: &mut *const u8) -> T {
    let val = *(*ptr as *const T);
    *ptr = ptr.add(size_of::<T>());
    val
}

pub static PAGE_SIZE: LazyLock<usize> = LazyLock::new(|| {
    let name = libc::_SC_PAGE_SIZE;
    let size = unsafe { libc::sysconf(name) };
    size as _
});

pub type Attr = bindings::perf_event_attr;
pub type Metadata = bindings::perf_event_mmap_page;
