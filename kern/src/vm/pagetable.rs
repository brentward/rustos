use core::iter::Chain;
use core::ops::{Deref, DerefMut, BitAnd, Sub};
use core::slice::Iter;
use core::slice::from_raw_parts_mut;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

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
        self.0.get_value(RawL3Entry::VALID) == EntryValid::Valid
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
    pub l3: [L3PageTable; 3],
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
                L3PageTable::new(),
            ],
        });

        page_table.l2.entries[0].set_masked(page_table.l3[0].as_ptr().as_u64(), RawL2Entry::ADDR);
        page_table.l2.entries[0].set_value(EntryType::Table, RawL2Entry::TYPE);
        page_table.l2.entries[0].set_value(EntryValid::Valid, RawL2Entry::VALID);

        page_table.l2.entries[1].set_masked(page_table.l3[1].as_ptr().as_u64(), RawL2Entry::ADDR);
        page_table.l2.entries[1].set_value(EntryType::Table, RawL2Entry::TYPE);
        page_table.l2.entries[1].set_value(EntryValid::Valid, RawL2Entry::VALID);

        page_table.l2.entries[2].set_masked(page_table.l3[2].as_ptr().as_u64(), RawL2Entry::ADDR);
        page_table.l2.entries[2].set_value(EntryType::Table, RawL2Entry::TYPE);
        page_table.l2.entries[2].set_value(EntryValid::Valid, RawL2Entry::VALID);

        page_table
    }

    /// Returns the (L2index, L3index) extracted from the given virtual address.
    /// L2index should be smaller than the number of L3PageTable.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address is not properly aligned to page size.
    /// Panics if extracted L2index exceeds the number of L3PageTable.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        if !allocator::util::has_alignment(va.as_usize(), Page::SIZE) {
            panic!("VirtualAddr: {} is not aligned to page size: {}", va.as_usize(), Page::SIZE);
        }
        let l2_index = va.bitand(VirtualAddr::from(0b1usize << 29)).as_usize() >> 29;
        let l3_index = va.bitand(VirtualAddr::from(0x1FFFusize << 16)).as_usize() >> 16;
        if l2_index >= 2 {
            panic!(
                "L2 Index: {} from VirtualAddr: {} is greater than L3PageTable count: 2",
                l2_index,
                va.as_usize()
            );
        }
        (l2_index, l3_index)
    }

    /// Returns `true` if the L3entry indicated by the given virtual address is valid.
    /// Otherwise, `false` is returned.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_index, l3_index) = PageTable::locate(va);
        let l2_entry = self.l2.entries[l2_index];
        // TODO: Make l3_addr usize
        let l3_addr = l2_entry.get_masked(RawL2Entry::ADDR);

        if self.l3[0].as_ptr().as_u64() == l3_addr {
            return self.l3[0].entries[l3_index].is_valid()
        } else if self.l3[1].as_ptr().as_u64() == l3_addr {
            return self.l3[1].entries[l3_index].is_valid()
        } else {
            panic!("Unexpected failure to find L3PageTable in PageTable::is_valid()")
        }
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
        let l3_addr = l2_entry.get_masked(RawL2Entry::ADDR);

        if self.l3[0].as_ptr().as_u64() == l3_addr {
            self.l3[0].entries[l3_index].0.set(entry.get())
        } else if self.l3[1].as_ptr().as_u64() == l3_addr {
            self.l3[1].entries[l3_index].0.set(entry.get())
        } else {
            panic!("Unexpected failure to find L3PageTable in PageTable::set_entry()")
        };

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
        let mem_start = 0x0000_0000usize;
        let (_, mem_end) = allocator::memory_map()
            .expect("unexpected None from allocator::memory_map()");

        for addr in (mem_start..IO_BASE_END).step_by(Page::SIZE) {
            let mut raw_l3_entry = RawL3Entry::new(0);
            raw_l3_entry.set_masked(addr as u64, RawL3Entry::ADDR);

            if addr < mem_end {
                raw_l3_entry.set_bit(RawL3Entry::AF);
                raw_l3_entry.set_value(EntrySh::ISh, RawL3Entry::SH);
                raw_l3_entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
                raw_l3_entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
                raw_l3_entry.set_value(PageType::Page, RawL3Entry::TYPE);
                raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            } else if addr >= IO_BASE && addr < IO_BASE_END {
                raw_l3_entry.set_bit(RawL3Entry::AF);
                raw_l3_entry.set_value(EntrySh::OSh, RawL3Entry::SH);
                raw_l3_entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
                raw_l3_entry.set_value(EntryAttr::Dev, RawL3Entry::ATTR);
                raw_l3_entry.set_value(PageType::Page, RawL3Entry::TYPE);
                raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);
            } else {
                raw_l3_entry.set_value(EntrySh::ISh, RawL3Entry::SH);
                raw_l3_entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
                raw_l3_entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
                raw_l3_entry.set_value(PageType::Page, RawL3Entry::TYPE);
                raw_l3_entry.set_value(EntryValid::Invalid, RawL3Entry::VALID);
            }

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

        let va_locate = va.sub(VirtualAddr::from(USER_IMG_BASE));
        // let va_locate = VirtualAddr::from(va.as_u64() - USER_IMG_BASE as u64);
        if self.is_valid(va_locate) {
            panic!("VirtualAddr already allocated");
        }

        let page_ptr = unsafe { ALLOCATOR.alloc(Page::layout()) };

        let mut raw_l3_entry = RawL3Entry::new(0);
        raw_l3_entry.set_masked(page_ptr as u64, RawL3Entry::ADDR);
        raw_l3_entry.set_bit(RawL3Entry::AF);
        raw_l3_entry.set_value(EntrySh::ISh, RawL3Entry::SH);
        raw_l3_entry.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        raw_l3_entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        raw_l3_entry.set_value(PageType::Page, RawL3Entry::TYPE);
        raw_l3_entry.set_value(EntryValid::Valid, RawL3Entry::VALID);

        self.set_entry(va_locate, raw_l3_entry);

        let page = unsafe { from_raw_parts_mut(page_ptr, PAGE_SIZE)} ;
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
        use alloc::string::String;
        use core::fmt::Write;

        let mut baddr = String::new();
        write!(baddr, "{:x}", &self.as_ptr().as_u64())?;

        f.debug_struct("L2PageTable")
            .field("baddr", &baddr)
            .field("entries.len()", &self.entries.len())
            .field("entry_0", &self.entries[0])
            .field("entry_1", &self.entries[1])

            .finish()
    }
}

impl fmt::Debug for L3PageTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use alloc::string::String;
        use core::fmt::Write;

        let mut baddr = String::new();
        write!(baddr, "{:x}", &self.as_ptr().as_u64())?;

        f.debug_struct("L3PageTable")
            .field("baddr", &baddr)
            .field("entries.len()", &self.entries.len())
            .field("entry_0", &self.entries[0].0)
            .field("entry_1", &self.entries[1].0)
            .field("entry_2", &self.entries[2].0)
            .field("entry_3", &self.entries[3].0)
            .field("entry_4", &self.entries[4].0)
            .field("entry_5", &self.entries[5].0)
            .field("entry_6", &self.entries[6].0)
            .field("entry_7", &self.entries[7].0)
            .field("entry_7935", &self.entries[7935].0)
            .field("entry_7936", &self.entries[7936].0)
            .field("entry_7937", &self.entries[7937].0)
            .field("entry_8191", &self.entries[8191].0)

            .finish()
    }
}
