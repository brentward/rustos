use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use crate::allocator::util::*;
use crate::allocator::linked_list::LinkedList;
use crate::LocalAlloc;
use kernel_api::syscall::sbrk;

const USER_VMM_ADDRESS_SIZE: usize= 30;
const BIN_COUNT_MAX: usize = USER_VMM_ADDRESS_SIZE - 3;

pub struct Allocator {
    current: usize,
    bins: [LinkedList; BIN_COUNT_MAX],
}

impl Allocator {
    /// Creates a new bin allocator that will allocate memory from the region
    /// starting at address `start` and ending at address `end`.
    pub fn new() -> Allocator {
        let current = sbrk(0).unwrap() as usize;
        let bins = [LinkedList::new(); BIN_COUNT_MAX];

        Allocator {
            current,
            bins,
        }
    }

    fn bin_size(index: usize) -> usize {
        2usize.pow(index as u32 + 3)
    }

    fn map_to_bin(&self, layout: &Layout) -> usize {
        let required_size = layout.size().max(layout.align());
        for index in 0..self.bins.len() {
            if Allocator::bin_size(index) >= required_size{
                return index
            }
        }
        panic!("layout will cause memory address overflow");
    }

}

impl LocalAlloc for Allocator {
    /// Allocates memory. Returns a pointer meeting the size and alignment
    /// properties of `layout.size()` and `layout.align()`.
    ///
    /// If this method returns an `Ok(addr)`, `addr` will be non-null address
    /// pointing to a block of storage suitable for holding an instance of
    /// `layout`. In particular, the block will be at least `layout.size()`
    /// bytes large and will be aligned to `layout.align()`. The returned block
    /// of storage may or may not have its contents initialized or zeroed.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure that `layout.size() > 0` and that
    /// `layout.align()` is a power of two. Parameters not meeting these
    /// conditions may result in undefined behavior.
    ///
    /// # Errors
    ///
    /// Returning null pointer (`core::ptr::null_mut`)
    /// indicates that either memory is exhausted
    /// or `layout` does not meet this allocator's
    /// size or alignment constraints.
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let index =  self.map_to_bin(&layout);
        for node in self.bins[index].iter_mut() {
            if has_alignment(node.value() as usize, layout.align()) {
                return node.pop() as *mut u8
            }
        }
        let aligned_addr = align_up(self.current, Allocator::bin_size(index));

        let alloc_end = aligned_addr + Allocator::bin_size(index);
        let request_size = alloc_end - self.current;
        match sbrk(request_size) {
            Ok(_ptr) => {
                self.current = alloc_end;
                aligned_addr as *mut u8
            }
            Err(_e) => core::ptr::null_mut()
        }
    }

    /// Deallocates the memory referenced by `ptr`.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure the following:
    ///
    ///   * `ptr` must denote a block of memory currently allocated via this
    ///     allocator
    ///   * `layout` must properly represent the original layout used in the
    ///     allocation call that returned `ptr`
    ///
    /// Parameters not meeting these conditions may result in undefined
    /// behavior.
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let index = self.map_to_bin(&layout);
        self.bins[index].push(ptr as *mut usize)
    }
}

impl fmt::Debug for Allocator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Allocator {{")?;
        writeln!(f, "  current: {}", self.current)?;
        for i in 0..self.bins.len() {
            writeln!(
                f,
                "  bin#{} size={} = {:#?}",
                i,
                Allocator::bin_size(i),
                self.bins[i]
            )?;
        }
        writeln!(f, "}}")?;

        Ok(())
    }
}
