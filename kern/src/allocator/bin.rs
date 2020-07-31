use core::alloc::Layout;
use core::fmt;
use core::ptr;
use core::mem;

use crate::allocator::linked_list::LinkedList;
use crate::allocator::util::*;
use crate::allocator::LocalAlloc;
use crate::param::*;

/// A simple allocator that allocates based on size classes.
///   bin 0 (2^3 bytes)    : handles allocations in (0, 2^3]
///   bin 1 (2^4 bytes)    : handles allocations in (2^3, 2^4]
///   ...
///   bin 29 (2^22 bytes): handles allocations in (2^31, 2^32]
///   
///   map_to_bin(size) -> k
///

const KERNEL_VMM_ADDRESS_SIZE: usize= mem::size_of::<usize>() * 8 - KERNEL_MASK_BITS;
const BIN_COUNT_MAX: usize = KERNEL_VMM_ADDRESS_SIZE - 3;

pub struct Allocator {
    fragmentation: usize,
    total_mem: usize,
    current: usize,
    end: usize,
    bins: [LinkedList; BIN_COUNT_MAX],
}

impl Allocator {
    /// Creates a new bin allocator that will allocate memory from the region
    /// starting at address `start` and ending at address `end`.
    pub fn new(start: usize, end: usize) -> Allocator {
        let current = start;
        let total_mem = end - current;
        let bins = [LinkedList::new(); BIN_COUNT_MAX];
        let fragmentation = 0;

        Allocator {
            fragmentation,
            total_mem,
            current,
            end,
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
        panic!(
            "layout will cause memory address overflow, size: {}, align: {}",
            layout.size(),
            layout.align()
        );
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
        if aligned_addr + Allocator::bin_size(index) > self.end {
            core::ptr::null_mut()
        } else {
            self.fragmentation += aligned_addr - self.current;
            self.current = aligned_addr + Allocator::bin_size(index);
            aligned_addr as *mut u8
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
        writeln!(f, "  fragmentation: {}", self.fragmentation)?;
        writeln!(f, "  current: {}", self.current)?;
        writeln!(f, "  end: {}", self.end)?;
        writeln!(f, "  unallocated mem: {}", self.end - self.current)?;
        writeln!(f, "  total mem: {}", self.total_mem)?;
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
