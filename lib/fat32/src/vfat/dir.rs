use alloc::string::String;
use alloc::vec::Vec;
use core::marker::{PhantomData, Copy};

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;
use core::char::{decode_utf16, REPLACEMENT_CHARACTER};

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
    cluster_address_high: u16,
    modification_timestamp: Timestamp,
    cluster_address_low: u16,
    file_size: u32,
}

impl VFatRegularDirEntry {
    fn cluster(&self) -> u32 {
        self.cluster_address_low as u32
            + self.cluster_address_high as u32 * 0x10000

        // self.cluster_address_low[0] as u32
        //     + self.cluster_address_low[1] as u32 * 0x100
        //     + self.cluster_address_high[0] as u32 * 0x10000
        //     + self.cluster_address_high[1] as u32 * 0x1000000
    }

    fn is_dir(&self) -> bool {
        self.attributes.value() & 0x10 != 0
    }
}

const_assert_size!(VFatRegularDirEntry, 32);


#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatLfnDirEntry {
    sequence_number: u8,
    name_1: [u16; 5],
    attributes: u8,
    lfn_type: u8,
    dos_fn_checksum: u8,
    name_2: [u16; 6],
    always_zero: u16,
    name_3: [u16; 2],
}

impl VFatLfnDirEntry {
    fn is_deleted(&self) -> bool {
        self.sequence_number & 0xE5 != 0
    }


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
    position: usize,
    vfat: HANDLE
}

impl<HANDLE: VFatHandle> DirIterator<HANDLE> {
    fn lfn(lfn_vec: &mut Vec<&VFatLfnDirEntry>) -> String {
        lfn_vec.sort_by_key(|lfn| lfn.sequence_number);
        let mut name: Vec<u16>  = Vec::with_capacity(lfn_vec.len() * 13);
        for vec in lfn_vec.iter() {
            name.extend_from_slice(&vec.name_1);
            name.extend_from_slice(&vec.name_2);
            name.extend_from_slice(&vec.name_3);
        }
        for index in name.len() - 13..name.len() {
            if name[index] == 0x0000 || name[index] == 0x00FF {
                name.resize(index, 0);
                break
            }
        }
        decode_utf16(name.iter().cloned())
            .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
            .collect::<String>()
    }

    fn short_name(file_name: &[u8; 8], file_ext: &[u8; 3]) -> String {
        let mut file_name_end = file_name.len();
        for position in 0usize..file_name_end {
            if file_name[position] == 0x00 || file_name[position] == 0x20 {
                file_name_end = position;
                break
            };
        }
        let mut file_ext_end = file_ext.len();
        for position in 0usize..file_ext_end {
            if file_ext[position] == 0x00 || file_ext[position] == 0x20 {
                file_ext_end = position
            };
        }
        let short_filename = core::str::from_utf8(&file_name[0..file_name_end])
            .expect("file name not utf8");
        let short_ext = core::str::from_utf8(&file_ext[0..file_ext_end])
            .expect("file ext not utf8");

        format!("{}.{}", short_filename, short_ext)
    }
}

impl<HANDLE: VFatHandle> Iterator for DirIterator<HANDLE> {
    type Item = Entry<HANDLE>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut lfn_vec: Vec<&VFatLfnDirEntry> = Vec::with_capacity(20);
        for position in self.position..self.dir_entries.len() {
            let dir_entry = &self.dir_entries[position];

            let unknown_dir_entry = unsafe {dir_entry.unknown};
            if unknown_dir_entry.is_end() {
                self.position = self.dir_entries.len();
                return None
            }
            if unknown_dir_entry.is_unused() {
                continue
            }
            if unknown_dir_entry.is_lfn() {
                lfn_vec.push(unsafe { &dir_entry.long_filename });
                self.position += 1;
                continue
            } else {
                let regular_dir = unsafe { dir_entry.regular };
                let name = if lfn_vec.len() == 0 {
                    Self::lfn(&mut lfn_vec)
                } else {
                    Self::short_name(&regular_dir.file_name, &regular_dir.file_ext)
                };
                let metadata = Metadata::new(
                    regular_dir.attributes,
                    regular_dir.creation_timestamp,
                    regular_dir.accessed_date,
                    regular_dir.modification_timestamp
                );
                self.position += 1;
                if regular_dir.is_dir(){
                    Some(Entry::Dir(
                        Dir {
                            vfat: self.vfat.clone(),
                            first_cluster: Cluster::from(regular_dir.cluster()),
                        },
                        name,
                        metadata,
                    ))

                } else {
                    Some(Entry::File(
                        File {
                            vfat: self.vfat.clone(),
                            first_cluster: Cluster::from(regular_dir.cluster()),
                            size: regular_dir.file_size,
                        },
                        name,
                        metadata,
                    ))
                }

            };

        }
        None
    }
}


impl<'a, HANDLE: VFatHandle + Copy> traits::Dir for Dir<HANDLE> {
    type Entry = Entry<HANDLE>;
    type Iter = DirIterator<HANDLE>;

    fn entries(&self) -> io::Result<Self::Iter> {
        let mut cluster_chain: Vec<u8> = Vec::new();
        self.vfat.lock(|a| a.read_chain(self.first_cluster, &mut cluster_chain))?;
        Ok(DirIterator {
            phantom: PhantomData,
            dir_entries: unsafe { cluster_chain.cast() },
            position: 0,
            vfat: self.vfat.clone(),
        })
    }
}
