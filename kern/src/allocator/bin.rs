use core::alloc::Layout;
use core::fmt;
use core::ptr;
use core::mem;

use crate::allocator::linked_list::LinkedList;
use crate::allocator::util::*;
use crate::allocator::LocalAlloc;

/// A simple allocator that allocates based on size classes.
///   bin 0 (2^3 bytes)    : handles allocations in (0, 2^3]
///   bin 1 (2^4 bytes)    : handles allocations in (2^3, 2^4]
///   ...
///   bin 29 (2^22 bytes): handles allocations in (2^31, 2^32]
///   
///   map_to_bin(size) -> k
///

const BLOCK_SIZE_COUNT: usize = mem::size_of::<usize>() * 8 - 3;

// #[derive(Debug)]
pub struct Allocator {
    fragmentation: usize,
    total_mem: usize,
    max_block_size: usize,
    bin_count: usize,
    current: usize,
    end: usize,
    bins: [LinkedList; BLOCK_SIZE_COUNT],
}

impl Allocator {
    /// Creates a new bin allocator that will allocate memory from the region
    /// starting at address `start` and ending at address `end`.
    pub fn new(start: usize, end: usize) -> Allocator {
        let current = start;
        let total_mem = end - current;
        let max_block_size = 1 << (mem::size_of::<usize>() * 8 - total_mem.leading_zeros() as usize - 1);
        let bin_count = (max_block_size as u64).trailing_zeros() as usize - 2;
        let bins = [LinkedList::new(); BLOCK_SIZE_COUNT];
        // let mut bins = [LinkedList::new(); BLOCK_SIZE_COUNT];
        let fragmentation = 0;
        // let current = align_up(current, max_block_size);
        // let fragmentation = current - start;
        // let (max_block_size, bin_count, current) = if end - current >= max_block_size {
        //     (max_block_size, bin_count, current)
        // } else {
        //     let bin_count = bin_count - 1;
        //     let max_block_size = Allocator::bin_size(bin_count - 1);
        //     let current = align_up(start, max_block_size);
        //     (max_block_size, bin_count, current)
        // };
        // unsafe { bins[bin_count - 1].push(current as *mut usize) };
        // let current = current + max_block_size;

        Allocator {
            fragmentation,
            total_mem,
            max_block_size,
            bin_count,
            current,
            end,
            bins,
        // };
        // let layout = &Layout::from_size_align(2usize, 8).unwrap();
        // match bin_allocator.populate_from_above(0, layout) {
        //     Some(addr) => {
        //         unsafe { bin_allocator.bins[0].push(addr) };
        //         bin_allocator
        //     },
        //     None => panic!("Nothing was returned from populate from above"),
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

    fn split_push_return(&mut self,
                         option: Option<*mut usize>,
                         index: usize,
                         layout: &Layout) -> Option<*mut usize> {
        match option {
            Some(addr) => {
                let (low, high) = unsafe {
                    split_addr(addr, Allocator::bin_size(index))
                };
                let low_align = align_up(low as usize, layout.align());
                let high_align = align_up(high as usize, layout.align());
                let closest = if low_align - low as usize <= high_align - high as usize {
                    unsafe {
                        self.bins[index].push(high);
                        low
                    }
                } else {
                    unsafe {
                        self.bins[index].push(low);
                        high
                    }
                };
                Some(closest)
            }
            None => None

        }
    }

    /// Pops an item from the list containing block sises one larger than
    /// `index`, splits it in half and checks for the half closest alignment
    /// to the `layout` and returns it as `Some(*mut usize)` while pushing the other
    /// into the list at `index`. If none are above it will search up the
    /// list recursively. `None` will be returned if none of the lists have members.
    ///
    /// The effect of this that all lists between `index` and the next highest list
    /// with that is not empty will get 1 item if `Some` is returned and no change
    /// if `None is returned.
    fn populate_from_above(&mut self, index: usize, layout: &Layout) -> Option<*mut usize> {
        if index < self.bin_count {
            if self.bins[index + 1].is_empty() {
                let option = self.populate_from_above(index + 1, layout);
                match self.split_push_return(option, index, layout) {
                    None => None,
                    some => some,
                }
            } else {
                let option = self.bins[index + 1].pop();
                match self.split_push_return(option, index, layout) {
                    None => None,
                    some => some,
                }
            }
        } else {
            None
        }
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
        match self.populate_from_above(index, &layout) {
            Some(addr) => {
                if has_alignment(addr as usize, layout.align()) {
                    addr as *mut u8
                } else {
                    core::ptr::null_mut()
                }
            },
            None => {
                if aligned_addr + Allocator::bin_size(index) > self.end {
                    core::ptr::null_mut()
                } else {
                    self.fragmentation += aligned_addr - self.current;
                    self.current = aligned_addr + Allocator::bin_size(index);
                    aligned_addr as *mut u8
                }
            }
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
        writeln!(f, "  max block size: {}", self.max_block_size)?;
        writeln!(f, "  sized block count: {}", self.bin_count)?;
        for i in 0..self.bin_count {
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
