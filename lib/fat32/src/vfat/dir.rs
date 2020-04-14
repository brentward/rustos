use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use shim::path::{Path, PathBuf};
use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io::{self, SeekFrom};
use shim::ioerr;
use shim::newioerr;
use core::char::{decode_utf16, REPLACEMENT_CHARACTER}; // TODO refactor this use
use core::mem::transmute;
use core::fmt;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFatHandle, Status};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: usize,
    pub path: PathBuf,
    pub parent_path: Option<PathBuf>,
    pub parent_first_cluster: Option<Cluster>,
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

    pub fn size(&self) -> usize {
        self.file_size as usize
    }

    pub fn set_size(&mut self, size: usize) -> io::Result<usize>{
        if size <= core::u32::MAX as usize {
            self.file_size = size as u32;
            Ok(self.size() as usize)
        } else {
            ioerr!(InvalidInput, "over maximum file size for FAT32")
        }
    }

    pub fn add_size(&mut self, size: usize) -> io::Result<usize> {
        let new_size = self.size() + size;
        self.set_size(new_size)
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

    fn is_regular(&self) -> bool {
        self.attributes != 0x0F
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

pub struct EntryModify {
    pub name: String,
    pub size: usize,
    pub modified: Timestamp,
}

pub struct DirIterator<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    dir_entries: Vec<VFatDirEntry>,
    position: usize,
    vfat: HANDLE,
    path: PathBuf,
    first_cluster: Cluster,
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

    pub fn write_entry_size(&mut self, name: &str, size: usize, buf: &mut Vec<u8>) -> io::Result<usize> {
        use crate::traits::Entry;
        use crate::util::SliceExt;
        use io::Write;

        self.position = 0;
        let mut bytes_writen = 0usize;
        let mut lfn_vec: Vec<&VFatLfnDirEntry> = Vec::with_capacity(20);

        for position in self.position..self.dir_entries.len() {
            let dir_entry = &self.dir_entries[position];

            let unknown_dir_entry = unsafe { dir_entry.unknown };
            if unknown_dir_entry.is_end() {
                let data_entry = unsafe {
                    transmute::<VFatUnknownDirEntry, [u8; 32]>(unknown_dir_entry)
                };
                let mut write_buf = &mut buf[bytes_writen..];
                let bytes = write_buf.write(&data_entry)?;
                bytes_writen += bytes;

                self.position = self.dir_entries.len();
                return Ok(bytes_writen)
            }
            if unknown_dir_entry.is_unused() {
                let data_entry = unsafe {
                    transmute::<VFatUnknownDirEntry, [u8; 32]>(unknown_dir_entry)
                };
                // let mut write_buf = &mut buf[bytes_writen..];
                buf.reserve(32);
                let bytes = buf.write(&data_entry)?;
                bytes_writen += bytes;

                self.position += 1;
            }
            if unknown_dir_entry.is_lfn() {
                lfn_vec.push(unsafe { &dir_entry.long_filename });
                let data_entry = unsafe {
                    transmute::<VFatUnknownDirEntry, [u8; 32]>(unknown_dir_entry)
                };
                // let mut write_buf = &mut buf[bytes_writen..];
                buf.reserve(32);
                let bytes = buf.write(&data_entry)?;
                bytes_writen += bytes;

                self.position += 1;
            } else {
                let mut regular_dir_entry = unsafe { dir_entry.regular };
                let entry_name = if lfn_vec.len() != 0 {
                    Self::lfn(&mut lfn_vec)
                } else {
                    Self::short_name(&regular_dir_entry.file_name, &regular_dir_entry.file_ext)
                };
                if entry_name.as_str() == name {
                    regular_dir_entry.set_size(size)?;
                }
                let data_entry = unsafe {
                    transmute::<VFatRegularDirEntry, [u8; 32]>(regular_dir_entry)
                };
                // let mut write_buf = &mut buf[bytes_writen..];
                buf.reserve(32);
                let bytes = buf.write(&data_entry)?;
                bytes_writen += bytes;

                self.position += 1;

            }

        }
        Ok(bytes_writen)

    // let mut dir_entries = self.dir_entries.clone();
        // let mut entry_index_option: Option<usize>  = None;
        // for entry in self {
        //     if entry.name() == name {
        //         let index = self.position - 1;
        //         entry_index_option = Some(index);
        //     }
        // }
        // let entry_index = match entry_index_option {
        //     Some(index) => index,
        //     None => return ioerr!(NotFound, "Canno find DirEntry"),
        // };
        // let mut dir_entry = match dir_entries.get_mut(entry_index) {
        //     Some(dir_entry) => dir_entry,
        //     None => return ioerr!(NotFound, "Canno find DirEntry"),
        // };
        // let unknown_dir_entry = unsafe { dir_entry.unknown };
        // if unknown_dir_entry.is_regular() {
        //     let mut regular_dir = unsafe { dir_entry.regular };
        //     let size = regular_dir.set_size(size)?;
        //     self.write_dir_entries()?;
        //     return Ok(size)
        // } else {
        //     return ioerr!(InvalidData, "VFatLfnDirEntry is invalid at this position")
        // }
        //
        // ioerr!(NotFound, "No entry matching name found in DirIterator")
    }

    // pub fn write_dir_entries(&mut self) -> io::Result<()> {
    //     let mut bytes_written = 0usize;
    //     let data: Vec<u8> = unsafe { self.dir_entries.clone().cast() };
    //     let bytes_to_write = data.len();
    //     let mut cluster = self.first_cluster;
    //     let bytes_per_cluster = self.vfat.lock(
    //         |vfat| vfat.bytes_per_cluster()
    //     );
    //     let mut current_cluster_result = Ok(Some((self.first_cluster)));
    //     let mut current_cluster = current_cluster_result?;
    //     while self.vfat.lock(
    //         |vfat| { vfat.size_to_chain_end(cluster.fat_address()) }
    //     )? < bytes_to_write {
    //         let new_cluster = self.vfat.lock(
    //             |vfat| {
    //                 vfat.find_free_cluster()
    //             }
    //         )?;
    //         self.vfat.lock(
    //             |vfat| {
    //                 vfat.add_cluster_to_chain(cluster, new_cluster)
    //             }
    //         )?;
    //         cluster = new_cluster;
    //     }
    //     while bytes_written < bytes_to_write {
    //         let bytes = self.vfat.lock(
    //             |vfat| {
    //                 vfat.write_cluster(
    //                     current_cluster.unwrap(),
    //                     0,
    //                     &data[bytes_written..]
    //                 )
    //             }
    //         )?;
    //         if bytes == bytes_per_cluster {
    //             current_cluster_result = self.vfat.lock(|vfat| {
    //                 match vfat.fat_entry(current_cluster.unwrap())?.status() {
    //                     Status::Data(cluster) => Ok(Some(cluster)),
    //                     Status::Eoc(_) => Ok(None),
    //                     Status::Bad => return ioerr!(InvalidInput, "cluster in chain marked bad"),
    //                     Status::Reserved => {
    //                         return ioerr!(InvalidInput, "cluster in chain marked reserved")
    //                     }
    //                     Status::Free => return ioerr!(InvalidInput, "cluster in chain marked free"),
    //                 }
    //             });
    //             current_cluster = current_cluster_result?;
    //
    //         } else {
    //             return ioerr!(UnexpectedEof, "Bytes written did not match cluster size")
    //         }
    //         bytes_written += bytes;
    //     }
    //     Ok(())
    // }
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

            let unknown_dir_entry = unsafe { dir_entry.unknown };
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
                let mut next_path = self.path.clone();
                next_path.push(name.as_str());
                return if regular_dir.is_dir() {
                    Some(Entry::Dir(
                        Dir {
                            vfat: self.vfat.clone(),
                            first_cluster: Cluster::from(regular_dir.cluster()),
                            name,
                            metadata,
                            size: regular_dir.file_size as usize,
                            path: next_path,
                            parent_path: Some(self.path.clone()),
                            parent_first_cluster: Some(self.first_cluster),
                        }
                    ))

                } else {
                    Some(Entry::File(File::from(
                        self.vfat.clone(),
                        Cluster::from(regular_dir.cluster()),
                        name,
                        metadata,
                        regular_dir.file_size as usize,
                        self.path.clone(),
                        self.first_cluster,
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
            path: self.path.clone(),
            first_cluster: self.first_cluster,
        })
    }
}
