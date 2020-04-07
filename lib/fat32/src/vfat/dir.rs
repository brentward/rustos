use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io::{self, SeekFrom};
use shim::ioerr;
use shim::newioerr;
use core::char::{decode_utf16, REPLACEMENT_CHARACTER};
use core::fmt;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: usize,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatRegularDirEntry {
    file_name: [u8; 8],
    file_ext: [u8; 3],
    attributes: u8,
    reserved_nt: u8,
    creation_time_tenth_of_second: u8,
    creation_time: u16,
    creation_date: u16,
    accessed_date: u16,
    cluster_address_high: u16,
    modification_time: u16,
    modification_date: u16,
    cluster_address_low: u16,
    file_size: u32,
}

impl VFatRegularDirEntry {
    fn cluster(&self) -> u32 {
        ((self.cluster_address_high as u32) << 16) | self.cluster_address_low as u32
    }

    fn is_dir(&self) -> bool {
        self.attributes & 0x10 != 0
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
    pub fn sequence_number(&self) -> usize {
        let sequence = self.sequence_number & 0x1F;
        sequence as usize
    }

    pub fn last_entry(&self) -> bool {
        self.sequence_number & 0x40 != 0
    }

    fn is_deleted(&self) -> bool {
        self.sequence_number & 0xE5 != 0
    }

    fn name_1(&self) -> &[u16; 5] {
        unsafe { &self.name_1 }
    }

    fn name_2(&self) -> &[u16; 6] {
        unsafe { &self.name_2 }
    }

    fn name_3(&self) -> &[u16; 2] {
        unsafe { &self.name_3 }
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
        use traits::{Dir, Entry};
        let name = name.as_ref().to_str()
            .ok_or(newioerr!(InvalidInput, "name is not valid UTF-8"))?;
        self.entries()?.find(|entry| {
            let entry_name = entry.name();
            entry_name.eq_ignore_ascii_case(name)
        }).ok_or(newioerr!(NotFound, "name was not found"))
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
        lfn_vec.sort_by_key(|lfn| lfn.sequence_number());
        let mut name: Vec<u16>  = Vec::with_capacity(lfn_vec.len() * 13);
        for lfn in lfn_vec.iter() {
            name.extend_from_slice(&lfn.name_1()[..]);
            name.extend_from_slice(&lfn.name_2()[..]);
            name.extend_from_slice(&lfn.name_3()[..]);
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
                file_ext_end = position;
                break
            };
        }
        let short_filename = core::str::from_utf8(&file_name[0..file_name_end])
            .expect("file name not utf8");
        let short_ext = core::str::from_utf8(&file_ext[0..file_ext_end])
            .expect("file ext not utf8");
        if short_ext.len() > 0 {
            format!("{}.{}", short_filename, short_ext)
        } else {
            format!("{}", short_filename)
        }
    }
}

impl<HANDLE: VFatHandle> io::Seek for  DirIterator<HANDLE> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {

        match pos {
            SeekFrom::Start(offset) => {
                if offset > self.dir_entries.len() as u64 {
                    ioerr!(InvalidInput, "beyond end of dir")
                } else {
                    self.position = offset as usize;
                    Ok(self.position as u64)
                }
            }
            SeekFrom::End(offset) => {
                if self.dir_entries.len() as i64 + offset < 0 {
                    ioerr!(InvalidInput, "beyond beginning of dir")
                } else {
                    self.position = (self.dir_entries.len() as i64 + offset) as usize;
                    Ok(self.position as u64)
                }
            }
            SeekFrom::Current(offset) => {
                if self.position as i64 + offset < 0 {
                    ioerr!(InvalidInput, "beyond beginning of dir")
                } else if self.position as i64 + offset > self.dir_entries.len() as i64 {
                    ioerr!(InvalidInput, "beyond end of dir")
                } else {
                    self.position = (self.position as i64 + offset) as usize;
                    Ok(self.position as u64)
                }
            }
        }

    }
}

impl<HANDLE: VFatHandle> fmt::Debug for DirIterator<HANDLE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DirIterator")
            .field("len", &self.dir_entries.len())
            .field("positon", &self.position)
            .finish()
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
            } else {
                self.position = position + 1;

                let regular_dir = unsafe { dir_entry.regular };
                let name = if lfn_vec.len() != 0 {
                    Self::lfn(&mut lfn_vec)
                } else {
                    Self::short_name(&regular_dir.file_name, &regular_dir.file_ext)
                };
                let metadata = Metadata::from(
                    (
                        regular_dir.attributes,
                        [
                            regular_dir.creation_date,
                            regular_dir.creation_time,
                            regular_dir.accessed_date,
                            regular_dir.modification_date,
                            regular_dir.modification_time
                        ]
                    )
                );
                return if regular_dir.is_dir(){
                    Some(Entry::Dir(
                        Dir {
                            vfat: self.vfat.clone(),
                            first_cluster: Cluster::from(regular_dir.cluster()),
                            name,
                            metadata,
                            size: regular_dir.file_size as usize,
                        }
                    ))

                } else {
                    Some(Entry::File(File::from(
                        self.vfat.clone(),
                        Cluster::from(regular_dir.cluster()),
                        name,
                        metadata,
                        regular_dir.file_size as usize,
                    )))
                }
            }
        }
        None
    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    type Entry = Entry<HANDLE>;
    type Iter = DirIterator<HANDLE>;

    fn entries(&self) -> io::Result<Self::Iter> {
        let mut cluster_chain: Vec<u8> = Vec::new();
        self.vfat.lock(
            |vfat| vfat.read_chain(self.first_cluster, &mut cluster_chain)
        )?;
        Ok(DirIterator {
            phantom: PhantomData,
            dir_entries: unsafe { cluster_chain.cast() },
            position: 0,
            vfat: self.vfat.clone(),
        })
    }
}
