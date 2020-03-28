use core::iter::Chain;
use core::ops::{Deref, DerefMut, BitAnd, Sub};
use core::slice::Iter;
use core::slice::from_raw_parts_mut;

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
        PhysicalAddr::from(self as *const L2PageTable)
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
        self.0.get_masked(RawL3Entry::VALID) == 1
    }

    /// Extracts `ADDR` field of the L3Entry and returns as a `PhysicalAddr`
    /// if valid. Otherwise, return `None`.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(PhysicalAddr::from(self.0.get_masked(RawL3Entry::ADDR)))
        } else {
            None
        }
    }
}

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
        PhysicalAddr::from(self as *const L3PageTable)
    }
}

#[derive(Debug)]
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
        let mut page_table = Box::new(PageTable {
            l2: L2PageTable::new(),
            l3: [
                L3PageTable::new(),
                L3PageTable::new(),
            ],
        });

        // let mut l2_page_table = L2PageTable::new();
        // let l3_page_tables = [
        //     L3PageTable::new(),
        //     L3PageTable::new(),
        // ];

        // let entry_0_addr = page_table.l3[0].as_ptr().as_ptr() as u64;
        // let entry_1_addr = page_table.l3[0].as_ptr().as_ptr() as u64;


        // let entry_0_value = page_table.l3[0].as_ptr().as_u64() << 16
        //     | EntrySh::ISh << 8
        //     | perm << 6
        //     | EntryAttr::Mem << 2
        //     | EntryType::Block << 1
        //     | EntryValid::Valid;
        // let entry_1_value = page_table.l3[1].as_ptr().as_u64() << 16
        //     | EntrySh::ISh << 8
        //     | perm << 6
        //     | EntryAttr::Mem << 2
        //     | EntryType::Block << 1
        //     | EntryValid::Valid;
        //
        // page_table.l2.entries[0].set(entry_0_value);
        // page_table.l2.entries[1].set(entry_1_value);

        page_table.l2.entries[0].set_value(page_table.l3[0].as_ptr().as_u64(), RawL2Entry::ADDR);
        // page_table.l2.entries[0].set_bit(RawL2Entry::AF);
        page_table.l2.entries[0].set_value(EntrySh::ISh, RawL2Entry::SH);
        page_table.l2.entries[0].set_value(perm, RawL2Entry::AP);
        page_table.l2.entries[0].set_value(EntryAttr::Mem, RawL2Entry::ATTR);
        page_table.l2.entries[0].set_value(EntryType::Table, RawL2Entry::TYPE);
        page_table.l2.entries[0].set_value(EntryValid::Valid, RawL2Entry::VALID);

        page_table.l2.entries[1].set_value(page_table.l3[1].as_ptr().as_u64(), RawL2Entry::ADDR);
        // page_table.l2.entries[1].set_bit(RawL2Entry::AF);
        page_table.l2.entries[1].set_value(EntrySh::ISh, RawL2Entry::SH);
        page_table.l2.entries[1].set_value(perm, RawL2Entry::AP);
        page_table.l2.entries[1].set_value(EntryAttr::Mem, RawL2Entry::ATTR);
        page_table.l2.entries[1].set_value(EntryType::Table, RawL2Entry::TYPE);
        page_table.l2.entries[1].set_value(EntryValid::Valid, RawL2Entry::VALID);

        // l2_page_table.entries[0].set_masked(l3_page_tables[0].as_ptr().as_u64() << 16, RawL2Entry::ADDR);
        // l2_page_table.entries[0].set_masked(0b1 << 10, RawL2Entry::AF);
        // l2_page_table.entries[0].set_masked(EntrySh::ISh << 9, RawL2Entry::SH);
        // l2_page_table.entries[0].set_masked(perm << 6, RawL2Entry::AP);
        // l2_page_table.entries[0].set_masked(EntryAttr::Mem << 2, RawL2Entry::ATTR);
        // l2_page_table.entries[0].set_masked(EntryType::Block <<1, RawL2Entry::TYPE);
        // l2_page_table.entries[0].set_masked(EntryValid::Valid, RawL2Entry::VALID);
        //
        // l2_page_table.entries[1].set_masked(l3_page_tables[1].as_ptr().as_u64(), RawL2Entry::ADDR);
        // l2_page_table.entries[1].set_masked(0b1 << 10, RawL2Entry::AF);
        // l2_page_table.entries[1].set_masked(EntrySh::ISh << 9, RawL2Entry::SH);
        // l2_page_table.entries[1].set_masked(perm << 6, RawL2Entry::AP);
        // l2_page_table.entries[1].set_masked(EntryAttr::Mem << 2, RawL2Entry::ATTR);
        // l2_page_table.entries[1].set_masked(EntryType::Block <<1, RawL2Entry::TYPE);
        // l2_page_table.entries[1].set_masked(EntryValid::Valid, RawL2Entry::VALID);

        // l2_page_table.entries[0].set(
        //     RawL2Entry::ADDR & (l3_page_tables[0].as_ptr().as_u64() << 16)
        //         | RawL2Entry::AF & 0b1 << 10
        //         | RawL2Entry::SH & (EntrySh::ISh << 9)
        //         | RawL2Entry::AP & (perm << 6)
        //         | RawL2Entry::ATTR & (EntryAttr::Mem << 2)
        //         | RawL2Entry::TYPE & (EntryType::Block <<1)
        //         | RawL2Entry::VALID & EntryValid::Valid
        // );
        // l2_page_table.entries[1].set(
        //     RawL2Entry::ADDR & (l3_page_tables[1].as_ptr().as_u64() << 16)
        //         | RawL2Entry::AF & 0b1 << 10
        //         | RawL2Entry::SH & (EntrySh::ISh << 9)
        //         | RawL2Entry::AP & (perm << 6)
        //         | RawL2Entry::ATTR & (EntryAttr::Mem << 2)
        //         | RawL2Entry::TYPE & (EntryType::Block <<1)
        //         | RawL2Entry::VALID & EntryValid::Valid
        // );
        page_table
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
        // let l2_index = va.bitand(VirtualAddr::from(0b1usize << 29)).as_usize() >> 29;
        // let l3_index = va.bitand(VirtualAddr::from(0x1FFFusize << 16)).as_usize() >> 16;
        let l2_index = (va.as_usize() & 0b1usize << 29) >> 29;
        let l3_index = (va.as_usize() & 0x1FFFusize << 16) >> 16;
        (l2_index, l3_index)
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_index, l3_index) = PageTable::locate(va);
        let l2_entry = self.l2.entries[l2_index];
        let l3_addr = l2_entry.get_value(RawL2Entry::ADDR) as u64;

        if self.l3[0].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
            return self.l3[0].entries[l3_index].is_valid()
        } else if self.l3[1].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
            return self.l3[1].entries[l3_index].is_valid()
        } else {
            panic!("Unexpected failure to find L3PageTable in PageTable::set_entry()")
        }

        // for page_table in self.l3.iter() {
        //     if page_table.as_ptr().as_u64() & 0xFFFFFFFF == l3_address {
        //         let l3_entry = page_table.entries[l3_index];
        //         return l3_entry.is_valid()
        //     }
        // }
        // false
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
        let l3_addr = l2_entry.get_value(RawL2Entry::ADDR) as u64;
        // let page_table_ptr = l3_address as *mut L3PageTable;
        // let mut page_table = unsafe {
        //     page_table_ptr.as_mut().expect("L3Page table failed to unwrap")
        // };
        // let mut l3_entry = page_table.entries[l3_index];
        // l3_entry.0 = entry;

        if self.l3[0].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
            self.l3[0].entries[l3_index].0.set(entry.get())
        } else if self.l3[1].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
            self.l3[1].entries[l3_index].0.set(entry.get())
        } else {
            panic!("Unexpected failure to find L3PageTable in PageTable::set_entry()")
        };
        // let mut l3_entry = page_table.entries[l3_index];
        // l3_entry.0.set(entry.get());


        // for page_table in self.l3.iter_mut() {
        //     if page_table.as_ptr().as_u64() == l3_address {
        //         let mut l3_entry = page_table.entries[l3_index];
        //         l3_entry.0 = entry;
        //     }
        // }
        self
    }

    /// Returns a base address of the pagetable. The returned `PhysicalAddr` value
    /// will point the start address of the L2PageTable.
    pub fn get_baddr(&self) -> PhysicalAddr {
        self.l2.as_ptr()
    }
}

impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;
    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    fn into_iter(self) -> Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>> {
        self.l3[0].entries.iter().chain(self.l3[1].entries.iter())
    }
}

#[derive(Debug)]
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
        let mut page_table = PageTable::new(EntryPerm::KERN_RW);
        let mut addr = 0u64;
        let (_, mem_end) = allocator::memory_map()
            .expect("unexpected None from allocator::memory_map()");

        // for ref mut entry in &*page_table {
        //     if addr < mem_end {
        //         break
        //     }
        //     let (entry_attr, entry_sh) = if addr >= IO_BASE && addr < IO_BASE_END {
        //         (EntryAttr::Dev, EntrySh::OSh)
        //     } else {
        //         (EntryAttr::Mem, EntrySh::ISh)
        //     };
        //     let entry_value = (RawL2Entry::ADDR) & (addr << 16) as u64
        //         | 0b1 << 10
        //         | entry_sh << 8
        //         | EntryPerm::KERN_RW << 6
        //         | entry_attr << 2
        //         | EntryType::Table <<1
        //         | EntryValid::Valid;
        //     entry.0.set(entry_value);
        //
        // }
        //
        //
        loop {
            if addr >= mem_end as u64 {
                break
            }
            let mut raw_l3_entry = RawL3Entry::new(0);
            raw_l3_entry.set_masked(addr, RawL3Entry::ADDR);
            // raw_l3_entry.set_bit(RawL3Entry::AF);
            raw_l3_entry.set_value(EntrySh::ISh, RawL3Entry::SH);
            raw_l3_entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            raw_l3_entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
            raw_l3_entry.set_value(EntryType::Table, RawL3Entry::TYPE);
            raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);


            // let raw_l3_entry = RawL3Entry::new(
            //     (RawL3Entry::ADDR & addr << 16)
            //         | entry_sh << 8
            //         | EntryPerm::KERN_RW << 6
            //         | entry_attr << 2
            //         | EntryType::Table <<1
            //         | EntryValid::Valid
            // );
            page_table.set_entry(VirtualAddr::from(addr), raw_l3_entry);
            addr += PAGE_SIZE as u64;
        }
        addr = IO_BASE as u64;
        loop {
            if addr >= IO_BASE_END as u64 {
                break
            }
            let mut raw_l3_entry = RawL3Entry::new(0);
            raw_l3_entry.set_masked(addr, RawL3Entry::ADDR);
            // raw_l3_entry.set_bit(RawL3Entry::AF);
            raw_l3_entry.set_value(EntrySh::OSh, RawL3Entry::SH);
            raw_l3_entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            raw_l3_entry.set_value(EntryAttr::Dev, RawL3Entry::ATTR);
            raw_l3_entry.set_value(EntryType::Table, RawL3Entry::TYPE);
            raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);

            page_table.set_entry(VirtualAddr::from(addr), raw_l3_entry);
            addr += PAGE_SIZE as u64;
        }
        KernPageTable(page_table)
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

#[derive(Debug)]
pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// Returns a new `UserPageTable` containing a `PageTable` created with
    /// `USER_RW` permission.
    pub fn new() -> UserPageTable {
        UserPageTable(PageTable::new(EntryPerm::USER_RW))
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
        if va.as_usize() < USER_IMG_BASE {
            panic!("UserPageTable::alloc() called with VirtualAddr lower than {}", USER_IMG_BASE);
        }
        // let va_locate = va.sub(VirtualAddr::from(USER_IMG_BASE));
        let va_locate = VirtualAddr::from(va.as_u64() - USER_IMG_BASE as u64);
        if self.is_valid(va_locate) {
            panic!("VirtualAddr already allocated");
        }
        // let (l2_index, l3_index) = PageTable::locate(va_locate);
        // let l2_entry = self.l2.entries[l2_index];
        // let l3_addr = l2_entry.get_value(RawL2Entry::ADDR);
        // let page_table_ptr = l3_addr as *mut L3PageTable;
        // let mut l3_page_table = unsafe {
        //     page_table_ptr.as_mut().expect("L3Page table failed to unwrap")
        // };
        let page_ptr = unsafe { ALLOCATOR.alloc(Page::layout()) };
        let mut raw_l3_entry = RawL3Entry::new(0);
        raw_l3_entry.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        // raw_l3_entry.set_bit(RawL3Entry::AF);
        raw_l3_entry.set_value(EntrySh::ISh, RawL3Entry::SH);
        raw_l3_entry.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        raw_l3_entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        raw_l3_entry.set_value(EntryType::Table, RawL3Entry::TYPE);
        raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);

        self.set_entry(va_locate, raw_l3_entry);

        // if self.l3[0].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
        //     // &mut self.l3[0]
        //     if self.l3[0].entries[l3_index].is_valid() {
        //         panic!("VirtualAddr is already allocated")
        //     }
        //     self.l3[0].entries[l3_index].0.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        //     self.l3[0].entries[l3_index].0.set_value(EntrySh::ISh, RawL3Entry::SH);
        //     self.l3[0].entries[l3_index].0.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        //     self.l3[0].entries[l3_index].0.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        //     self.l3[0].entries[l3_index].0.set_value(EntryType::Table, RawL3Entry::TYPE);
        //     self.l3[0].entries[l3_index].0.set_value(EntryValid::Valid, RawL3Entry::VALID);
        // } else if self.l3[1].as_ptr().as_u64() & 0xFFFFFFFF == l3_addr {
        //     // &mut self.l3[1]
        //     if self.l3[0].entries[l3_index].is_valid() {
        //         panic!("VirtualAddr is already allocated")
        //     }
        //     self.l3[1].entries[l3_index].0.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        //     self.l3[1].entries[l3_index].0.set_value(EntrySh::ISh, RawL3Entry::SH);
        //     self.l3[1].entries[l3_index].0.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        //     self.l3[1].entries[l3_index].0.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        //     self.l3[1].entries[l3_index].0.set_value(EntryType::Table, RawL3Entry::TYPE);
        //     self.l3[1].entries[l3_index].0.set_value(EntryValid::Valid, RawL3Entry::VALID);
        // } else {
        //     panic!("Unexpected failure to find L3PageTable in UserPageTable::alloc()")
        // };
        // let mut l3_entry = l3_page_table.entries[l3_index];
        // if l3_entry.is_valid() {
        //     panic!("VirtualAddr is already allocated")
        // }

        // let entry_value = (((page_ptr as u64) << 16) as u64 & 0xFFFFFFFF << 16)
        //     | 0b1 << 10
        //     | EntrySh::ISh << 8
        //     | EntryPerm::USER_RW << 6
        //     | EntryAttr::Mem << 2
        //     | EntryType::Table << 1
        //     | EntryValid::Valid;
        //
        // l3_entry.0.set(entry_value);
        // l3_entry.0.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        // l3_entry.0.set_value(EntrySh::ISh, RawL3Entry::SH);
        // l3_entry.0.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        // l3_entry.0.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        // l3_entry.0.set_value(EntryType::Block, RawL3Entry::TYPE);
        // l3_entry.0.set_value(EntryValid::Valid, RawL3Entry::VALID);

        // l3_entry.0.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        // l3_entry.0.set_masked(0b1 << 10, RawL3Entry::AF);
        // l3_entry.0.set_masked(EntrySh::ISh << 9, RawL3Entry::SH);
        // l3_entry.0.set_masked(EntryPerm::USER_RW, RawL3Entry::AP);
        // l3_entry.0.set_masked(EntryAttr::Mem << 2, RawL3Entry::ATTR);
        // l3_entry.0.set_masked(EntryType::Table <<1, RawL3Entry::TYPE);
        // l3_entry.0.set_masked(EntryValid::Valid, RawL3Entry::VALID);

        let mut page = unsafe { from_raw_parts_mut(page_ptr, PAGE_SIZE)} ;
        page
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

impl Drop for UserPageTable {
    fn drop(&mut self) {
        for entry in &*self.0 {
            if entry.is_valid() {
                let mut pa = PhysicalAddr::from(entry.0.get_masked(RawL3Entry::ADDR));
                unsafe { ALLOCATOR.dealloc(pa.as_mut_ptr(), Page::layout()) };
            }
        }
    }
}

impl fmt::Debug for L2PageTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("L2PageTable")
            .field("baddr", &self.as_ptr().as_u64())
            .field("entries", &"<entry table>")
            .field("entry_0", &self.entries[0].get())
            .field("entry_1", &self.entries[1].get())

            .finish()
    }
}

impl fmt::Debug for L3PageTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("L3PageTable")
            .field("baddr", &self.as_ptr().as_u64())
            .field("entries", &"<entry table>")
            .field("entry_0", &self.entries[0].0.get())
            .field("entry_1", &self.entries[1].0.get())
            .field("entry_2", &self.entries[2].0.get())
            .field("entry_3", &self.entries[3].0.get())
            .field("entry_4", &self.entries[4].0.get())
            .field("entry_5", &self.entries[5].0.get())
            .field("entry_6", &self.entries[6].0.get())
            .field("entry_7", &self.entries[7].0.get())
            .field("entry_7935", &self.entries[7935].0.get())
            .field("entry_7936", &self.entries[7936].0.get())
            .field("entry_7937", &self.entries[7937].0.get())
            .field("entry_8191", &self.entries[8191].0.get())
            // .field("entry_8", &self.entries[8].0.get())
            // .field("entry_9", &self.entries[9].0.get())
            // .field("entry_10", &self.entries[10].0.get())
            // .field("entry_11", &self.entries[11].0.get())
            // .field("entry_12", &self.entries[12].0.get())
            // .field("entry_13", &self.entries[13].0.get())
            // .field("entry_14", &self.entries[14].0.get())
            // .field("entry_15", &self.entries[15].0.get())
            // .field("entry_16", &self.entries[16].0.get())
            // .field("entry_17", &self.entries[17].0.get())
            // .field("entry_18", &self.entries[18].0.get())
            // .field("entry_19", &self.entries[19].0.get())
            // .field("entry_20", &self.entries[20].0.get())
            // .field("entry_21", &self.entries[21].0.get())
            // .field("entry_22", &self.entries[22].0.get())
            // .field("entry_23", &self.entries[23].0.get())
            // .field("entry_24", &self.entries[24].0.get())
            // .field("entry_25", &self.entries[25].0.get())
            // .field("entry_26", &self.entries[26].0.get())
            // .field("entry_27", &self.entries[27].0.get())
            // .field("entry_28", &self.entries[28].0.get())
            // .field("entry_29", &self.entries[29].0.get())
            // .field("entry_30", &self.entries[30].0.get())
            // .field("entry_31", &self.entries[31].0.get())
            // .field("entry_32", &self.entries[32].0.get())
            // .field("entry_33", &self.entries[33].0.get())
            // .field("entry_34", &self.entries[34].0.get())
            // .field("entry_35", &self.entries[35].0.get())

            .finish()
    }
}

pub mod entry_mask {
    pub const ADDR: u64 = 0xFFFFFFFF << 16;
    pub const AF: u64 = 0b1 << 10;
    pub const SH: u64 = 0b11 << 8;
    pub const AP: u64 = 0b11 << 6;
    pub const ATTR: u64 = 0b111 << 2;
    pub const TYPE: u64 = 0b1 << 1;
    pub const VALID: u64 = 0b1;
}

// FIXME: Implement `Drop` for `UserPageTable`.
// FIXME: Implement `fmt::Debug` as you need.
