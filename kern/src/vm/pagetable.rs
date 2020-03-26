use core::iter::Chain;
use core::ops::{Deref, DerefMut, BitAnd};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};
use alloc::vec::Vec;

use crate::allocator;
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;

use aarch64::vmsa::*;
use shim::const_assert_size;

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// Returns a new `L2PageTable`
    fn new() -> L2PageTable {
        L2PageTable {
            entries: [RawL2Entry::new(0); 8192]
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L2PageTable as usize)
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// Returns a new `L3Entry`.
    fn new() -> L3Entry {
        L3Entry(RawL3Entry::new(0))
    }

    /// Returns `true` if the L3Entry is valid and `false` otherwise.
    fn is_valid(&self) -> bool {
        self.0.get_value(RawL3Entry::VALID) == 1
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(PhysicalAddr::from(self.0.get_value(RawL3Entry::ADDR)))
        } else {
            None
        }
    }
}

// #[derive(Copy, Clone)]
#[repr(C)]
#[repr(align(65536))]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// Returns a new `L3PageTable`.
    fn new() -> L3PageTable {
        L3PageTable {
            entries: [L3Entry::new(); 8192]
        }
    }

    /// Returns a `PhysicalAddr` of the pagetable.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self as *const L3PageTable as usize)
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 2],
}

impl PageTable {
    /// Returns a new `Box` containing `PageTable`.
    /// Entries in L2PageTable should be initialized properly before return.
    fn new(perm: u64) -> Box<PageTable> {
        let mut l2_page_table = L2PageTable::new();
        let l3_page_tables = [
            L3PageTable::new(),
            L3PageTable::new(),
        ];

        l2_page_table.entries[0].set(
            RawL2Entry::ADDR & (l3_page_tables[0].as_ptr().as_u64() << 16)
                | RawL2Entry::AF & 0b1 << 10
                | RawL2Entry::SH & (EntrySh::ISh << 9)
                | RawL2Entry::AP & (perm << 6)
                | RawL2Entry::ATTR & (EntryAttr::Mem << 2)
                | RawL2Entry::TYPE & (EntryType::Block <<1)
                | RawL2Entry::VALID & EntryValid::Valid
        );
        l2_page_table.entries[1].set(
            RawL2Entry::ADDR & (l3_page_tables[1].as_ptr().as_u64() << 16)
                | RawL2Entry::AF & 0b1 << 10
                | RawL2Entry::SH & (EntrySh::ISh << 9)
                | RawL2Entry::AP & (perm << 6)
                | RawL2Entry::ATTR & (EntryAttr::Mem << 2)
                | RawL2Entry::TYPE & (EntryType::Block <<1)
                | RawL2Entry::VALID & EntryValid::Valid
        );

        // l2_page_table.entries[0].set(RawL2Entry::ADDR & l3_page_tables[0].as_ptr().as_u64());
        // l2_page_table.entries[0].set(RawL2Entry::AF & 0b1);
        // l2_page_table.entries[0].set(RawL2Entry::SH & EntrySh::ISh);
        // l2_page_table.entries[0].set(RawL2Entry::AP & perm);
        // l2_page_table.entries[0].set(RawL2Entry::ATTR & EntryAttr::Mem);
        // l2_page_table.entries[0].set(RawL2Entry::TYPE & EntryType::Block);
        // l2_page_table.entries[0].set(RawL2Entry::VALID & EntryValid::Valid);
        // l2_page_table.entries[1].set(RawL2Entry::ADDR & l3_page_tables[1].as_ptr().as_u64());
        // l2_page_table.entries[1].set(RawL2Entry::AF & 0b1);
        // l2_page_table.entries[1].set(RawL2Entry::SH & EntrySh::ISh);
        // l2_page_table.entries[1].set(RawL2Entry::AP & perm);
        // l2_page_table.entries[1].set(RawL2Entry::ATTR & EntryAttr::Mem);
        // l2_page_table.entries[1].set(RawL2Entry::TYPE & EntryType::Block);
        // l2_page_table.entries[1].set(RawL2Entry::VALID & EntryValid::Valid);
        Box::new(PageTable {
            l2: l2_page_table,
            l3: l3_page_tables,
        })

    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// Since we are only supporting 1GB virtual memory in this system, L2index
    /// should be smaller than 2.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let l2_index = va.bitand(VirtualAddr::from(0b11usize << 29)).as_usize();
        let l3_index = va.bitand(VirtualAddr::from(0b1_1111_1111_1111usize << 16)).as_usize();
        (l2_index, l3_index)
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_index, l3_index) = PageTable::locate(va);
        let l2_entry = self.l2.entries[l2_index];
        let l3_address = l2_entry.get_masked(0xFFFFFFFF << 16) as u64;
        for page_table in self.l3.iter() {
            if page_table.as_ptr().as_u64() == l3_address {
                let l3_entry = page_table.entries[l3_index];
                return l3_entry.is_valid()
            }
        }
        false
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is invalid.
    /// Otherwise, `true` is returned.
    pub fn is_invalid(&self, va: VirtualAddr) -> bool {
        !self.is_valid(va)
    }

    /// Set the given RawL3Entry `entry` to the L3Entry indicated by the given virtual
    /// address.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2_index, l3_index) = PageTable::locate(va);
        let l2_entry = self.l2.entries[l2_index];
        let l3_address = l2_entry.get_masked(0xFFFFFFFF << 16) as u64;
        for page_table in self.l3.iter_mut() {
            if page_table.as_ptr().as_u64() == l3_address {
                let mut l3_entry = page_table.entries[l3_index];
                l3_entry.0 = entry;
            }
        }
        self
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        self.l2.as_ptr()
    }
}

// struct PageTableIter<'a>(Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>);

impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;
    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    fn into_iter(self) -> Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>> {
        self.l3[0].entries.iter().chain(self.l3[1].entries.iter())
    }
}

pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// Returns a new `KernPageTable`. `KernPageTable` should have a `Pagetable`
    /// created with `KERN_RW` permission.
    ///
    /// Set L3entry of ARM physical address starting at 0x00000000 for RAM and
    /// physical address range from `IO_BASE` to `IO_BASE_END` for peripherals.
    /// Each L3 entry should have correct value for lower attributes[10:0] as well
    /// as address[47:16]. Refer to the definition of `RawL3Entry` in `vmsa.rs` for
    /// more details.
    pub fn new() -> KernPageTable {
        // // let page_table = KernPageTable(PageTable::new(EntryPerm::KERN_RW));
        let mut page_table = PageTable::new(EntryPerm::KERN_RW);
        let mem_start = 0x0000_0000usize;
        let (_, mem_end) = allocator::memory_map()
            .expect("unexpected None from allocator::memory_map()");

        // for l3_entry in &*page_table {
        //     if mem_addr > mem_end {
        //         break
        //     }
        //     let entry_attr = if mem_addr >= IO_BASE && mem_addr < IO_BASE_END {
        //         EntryAttr::Dev
        //     } else {
        //         EntryAttr::Mem
        //     };
        //     l3_entry.0.set(
        //         RawL2Entry::ADDR & ((mem_addr << 16) as u64)
        //             | RawL2Entry::AF & 0b1 << 10
        //             | RawL2Entry::SH & (EntrySh::ISh << 9)
        //             | RawL2Entry::AP & (EntryPerm::KERN_RW << 6)
        //             | RawL2Entry::ATTR & (entry_attr << 2)
        //             | RawL2Entry::TYPE & (EntryType::Table <<1)
        //             | RawL2Entry::VALID & EntryValid::Valid
        //     );
        //     mem_addr += PAGE_SIZE;
        // }
        for addr in (mem_start..mem_end).step_by(PAGE_SIZE) {
            let entry_attr = if addr >= IO_BASE && addr < IO_BASE_END {
                EntryAttr::Dev
            } else {
                EntryAttr::Mem
            };

            let raw_l3_entry = RawL3Entry::new(
                RawL2Entry::ADDR & (addr << 16) as u64
                    | RawL2Entry::AF & 0b1 << 10
                    | RawL2Entry::SH & (EntrySh::ISh << 9)
                    | RawL2Entry::AP & (EntryPerm::KERN_RW << 6)
                    | RawL2Entry::ATTR & (entry_attr << 2)
                    | RawL2Entry::TYPE & (EntryType::Table <<1)
                    | RawL2Entry::VALID & EntryValid::Valid
            );
            page_table.set_entry(VirtualAddr::from(addr), raw_l3_entry);

        }
        KernPageTable(page_table)
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        unimplemented!("UserPageTable::new()")
    }

    /// Allocates a page and set an L3 entry translates given virtual address to the
    /// physical address of the allocated page. Returns the allocated page.
    ///
    /// # Panics
    /// Panics if the virtual address is lower than `USER_IMG_BASE`.
    /// Panics if the virtual address has already been allocated.
    /// Panics if allocator fails to allocate a page.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        unimplemented!("alloc()");
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// FIXME: Implement `Drop` for `UserPageTable`.
// FIXME: Implement `fmt::Debug` as you need.
