pub mod bindings;

// Dereferences the pointer and offsets by the size of the
// pointee type, then returns the dereferenced value.
#[inline]
pub unsafe fn deref_offset<T: Copy>(ptr: &mut *const u8) -> T {
    let val = *(*ptr as *const T);
    *ptr = ptr.add(size_of::<T>());
    val
}

pub type Attr = bindings::perf_event_attr;
