/// Align `addr` downwards to the nearest multiple of `align`.
///
/// The returned usize is always <= `addr.`
///
/// # Panics
///
/// Panics if `align` is not a power of 2.
pub fn align_down(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two());
    addr & !(align - 1)
}

/// Align `addr` upwards to the nearest multiple of `align`.
///
/// The returned `usize` is always >= `addr.`
///
/// # Panics
///
/// Panics if `align` is not a power of 2
/// or aligning up overflows the address.
pub fn align_up(addr: usize, align: usize) -> usize {
    align_down(addr + align - 1, align)
}

/// Returns `true` if `addr` is aligned to  `align` and `false` otherwise.
pub fn has_alignment(addr: usize, align: usize) -> bool {
    addr == addr & !(align - 1)
}

/// Takes one pointer `addr` and `size` and returns a tupple two pointers
/// of type `*mut usize` where the first is the size of size and the second is the remaining size
///
/// # Saftey
///
/// The caller must ensure that `addr` refers to unique, writeable memory at
/// least `size` in size.
pub unsafe fn split_addr(addr: *mut usize, size: usize) -> (*mut usize, *mut usize) {
    let new_addr = addr as usize + size;
    let new_ptr = new_addr as *mut usize;
    (addr, new_ptr)
}