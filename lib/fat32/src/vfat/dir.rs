use alloc::string::String;
use alloc::vec::Vec;
use core::marker::{PhantomData, Copy};

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatRegularDirEntry {
    file_name: [u8; 8],
    file_ext: [u8; 3],
    attributes: Attributes,
    reserved_nt: u8,
    creation_time_tenth_of_second: u8,
    creation_timestamp: Timestamp,
    accessed_date: Date,
    cluster_address_high: [u8; 2],
    modification_timestamp: Timestamp,
    cluster_address_low: [u8; 2],
    file_size: u32,
}

const_assert_size!(VFatRegularDirEntry, 32);


#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatLfnDirEntry {
    sequence_number: u8,
    name_1: [u8; 10],
    attributes: u8,
    lfn_type: u8,
    dos_fn_checksum: u8,
    name_2: [u8; 12],
    always_zero: [u8; 2],
    name_3: [u8; 4],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatUnknownDirEntry {
    id: u8,
    unknown_1: [u8; 10],
    attributes: u8,
    unknown_2: [u8; 20],
}

impl VFatUnknownDirEntry {
    fn is_lfn(&self) -> bool {
        self.attributes == 0x0F
    }

    fn is_unused(&self) -> bool {
        self.id == 0xE5
    }

    fn is_end(&self) -> bool {
        self.id == 0x00
    }

    // fn to_lfn_dir(&self) -> Result(VFatLfnDirEntry, ()) {
    //     if !self.is_lfn() {
    //         Err(())
    //     } else {
    //         Ok(unsafe { [self].cast()[0] })
    //     }
    // }
    //
    // fn to_regular_dir(&self) -> Result(VFatRegularDirEntry, ()) {
    //     if self.is_lfn() {
    //         Err(())
    //     } else {
    //         Ok(unsafe { [self].cast()[0] })
    //     }
    // }

}

const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        unimplemented!("Dir::find()")
    }
}

pub struct DirIterator<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    dir_entries: Vec<VFatDirEntry>,
    position: u32,
    vfat: HANDLE
}

impl<HANDLE: VFatHandle> Iterator for DirIterator<HANDLE> {
    type Item = Entry<HANDLE>;
    fn next(&mut self) -> Option<Self::Item> {
        panic!("DirIter::next()")
    }
}


impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE>
    where HANDLE: Copy
{
    type Entry = Entry<HANDLE>;
    type Iter = DirIterator<HANDLE>;

    fn entries(&self) -> io::Result<Self::Iter> {
        unimplemented!("Dir::entries")
    }
    // FIXME: Implement `trait::Dir` for `Dir`.
}
