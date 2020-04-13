use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::{size_of, transmute};


use alloc::vec::Vec;
use alloc::string::String;

use shim::io::{self, Write};
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::{Path, Component, PathBuf};

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status, Metadata};

/// A generic trait that handles a critical section as a closure
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    pub rootdir_cluster: Cluster,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
    where
        T: BlockDevice + 'static,
    {
        let mbr = MasterBootRecord::from(&mut device)?;

        let partition_entry = mbr.get_partition(0);
        if !partition_entry.is_fat32() {
            return Err(Error::NotFound)
        }
        let start_sector = partition_entry.start_sector() as u64;
        let ebpb = BiosParameterBlock::from(&mut device, start_sector)?;

        let partition = Partition {
            start: partition_entry.start_sector() as u64,
            num_sectors: partition_entry.total_sectors() as u64,
            sector_size: ebpb.bytes_per_sector()
        };

        let cached_partition = CachedPartition::new(device, partition);
        let bytes_per_sector = ebpb.bytes_per_sector() as u16;
        let sectors_per_cluster = ebpb.sectors_per_cluster();
        let sectors_per_fat = ebpb.sectors_per_fat();
        let fat_start_sector = ebpb.fat_start_sector();
        let data_start_sector = ebpb.data_start_sector();
        let rootdir_cluster = Cluster::from(ebpb.root_dir_cluster());
        let vfat = VFat {
            phantom: PhantomData,
            device: cached_partition,
            bytes_per_sector,
            sectors_per_cluster,
            sectors_per_fat,
            fat_start_sector,
            data_start_sector,
            rootdir_cluster
        };
        Ok(VFatHandle::new(vfat))
    }

    pub fn bytes_per_cluster(&self) -> usize {
        (self.bytes_per_sector * self.sectors_per_cluster as u16) as usize
    }

    pub fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        mut buf: &mut [u8]
    ) -> io::Result<usize> {
        let first_sector = self.data_start_sector
            + (cluster.data_address() as u64 * self.sectors_per_cluster as u64);
        let mut bytes = 0usize;
        let start_sector_index = offset / self.bytes_per_sector as usize;
        for sector_index in start_sector_index..self.sectors_per_cluster as usize  {
            if buf.len() == 0 {
                break
            }
            let current_offset = (offset + bytes)
                - (sector_index as usize * self.bytes_per_sector as usize);
            let data = self.device.get(first_sector + sector_index as u64)?;
            let bytes_written = buf.write(&data[current_offset..])?;
            bytes += bytes_written;
        }
        Ok(bytes)
    }

    pub fn write_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        buf: &[u8]
    ) -> io::Result<usize> {
        let first_sector = self.data_start_sector
            + (cluster.data_address() as u64 * self.sectors_per_cluster as u64);
        let mut bytes = 0usize;
        let start_sector_index = offset / self.bytes_per_sector as usize;
        for sector_index in start_sector_index..self.sectors_per_cluster as usize  {
            if buf.len() == bytes {
                break
            }
            let current_offset = (offset + bytes)
                - (sector_index as usize * self.bytes_per_sector as usize);
            let data = self.device.get_mut(first_sector + sector_index as u64)?;
            let mut write_buf = &mut data[current_offset..];
            let bytes_written = write_buf.write(&buf[bytes..])?;
            bytes += bytes_written;
        }
        Ok(bytes)
    }

    pub fn find_free_cluster(&mut self) -> io::Result<Cluster> {
        let total_clusters = self.device.get_sector_count() as u32 / self.sectors_per_cluster as u32;
        for raw_cluster in 0..total_clusters {
            let cluster = Cluster::from(raw_cluster);
            match self.fat_entry(cluster)?.status() {
                Status::Free => return Ok(cluster),
                _ => (),
            }
        }
        ioerr!(NotFound, "No free clusters")
    }

    fn add_cluster_to_buf(&mut self, cluster: Cluster, buf: &mut Vec<u8>) -> io::Result<usize> {
        let start = buf.len();
        let bytes_per_cluster =
            (self.sectors_per_cluster as u16 * self.bytes_per_sector) as usize;
        let max_size = start + bytes_per_cluster;
        buf.reserve(max_size);

        let mut cluster_chunk_buf = [0u8; 512];
        let mut bytes_total = 0usize;
        for cluster_chunk in 0usize..(bytes_per_cluster as usize / 512) {
            let bytes_read = self
                .read_cluster(cluster, cluster_chunk * 512, &mut cluster_chunk_buf)?;
            for &byte in cluster_chunk_buf[0..bytes_read].iter() {
                buf.push(byte);
            }
            bytes_total += bytes_read;
        }
        unsafe {
            buf.set_len(start + bytes_total);
        }
        Ok(bytes_total)
    }

    pub fn size_to_chain_end(&mut self, current_cluster_raw: u32) -> io::Result<usize> {
        let mut cluster = Cluster::from(current_cluster_raw);
        let bytes_per_cluster =
            self.sectors_per_cluster as usize * self.bytes_per_sector as usize;
        let mut size = 0usize;
        loop {
            let fat_entry = self.fat_entry(cluster)?.status();
            match fat_entry {
                Status::Data(next_cluster) => {
                    size += bytes_per_cluster;
                    cluster = next_cluster;
                },
                Status::Eoc(_) => {
                    size += bytes_per_cluster;
                    break
                },
                Status::Bad => return ioerr!(InvalidData, "cluster in chain marked bad"),
                Status::Reserved => return ioerr!(InvalidData, "cluster in chain marked reserved"),
                Status::Free => return ioerr!(InvalidData, "cluster in chain marked free"),
            };
        }
        Ok(size)
    }

    pub fn read_chain(
        &mut self,
        start: Cluster,
        buf: &mut Vec<u8>
    ) -> io::Result<usize> {
        let mut cluster = start;
        let mut bytes = 0usize;
        loop {
            let fat_entry = self.fat_entry(cluster)?.status();
            match fat_entry {
                Status::Data(next_cluster) => {
                    bytes += self.add_cluster_to_buf(cluster, buf)?;
                    cluster = next_cluster;
                },
                Status::Eoc(_) => {
                    bytes += self.add_cluster_to_buf(cluster, buf)?;
                    break
                },
                Status::Bad => return ioerr!(InvalidData, "cluster in chain marked bad"),
                Status::Reserved => return ioerr!(InvalidData, "cluster in chain marked reserved"),
                Status::Free => return ioerr!(InvalidData, "cluster in chain marked free"),
            };
        }
    Ok(bytes)
    }

    pub fn write_chain(
        &mut self,
        start: Cluster,
        buf: &Vec<u8>
    ) -> io::Result<usize> {
        let mut cluster = start;
        let mut bytes = 0usize;
        loop {
            if bytes >= buf.len() {
                break
            }
            let fat_entry = self.fat_entry(cluster)?.status();
            // let cluster_end = bytes + (self.bytes_per_sector * self.sectors_per_cluster as u16) as usize;
            match fat_entry {
                Status::Data(next_cluster) => {
                    bytes += self.write_cluster(cluster, 0, &buf[bytes..])?;
                    cluster = next_cluster;
                },
                Status::Eoc(_) => {
                    bytes += self.write_cluster(cluster, 0, &buf[bytes..])?;
                    break
                },
                Status::Bad => return ioerr!(InvalidData, "cluster in chain marked bad"),
                Status::Reserved => return ioerr!(InvalidData, "cluster in chain marked reserved"),
                Status::Free => return ioerr!(InvalidData, "cluster in chain marked free"),
            };
        }
        Ok(bytes)
    }

    pub fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let sector = cluster.fat_address() as u64 * 4 / self.bytes_per_sector as u64;
        let position_in_sector = cluster
            .fat_address() as usize * 4 - sector as usize * self.bytes_per_sector as usize;
        if sector > self.sectors_per_fat as u64 {
            return ioerr!(NotFound, "invalid cluster index: fat entry")
        }
        let data = self
            .device.get(self.fat_start_sector + sector)?;
        Ok(unsafe { &data[position_in_sector..position_in_sector + 4].cast()[0] })
    }

    pub fn add_cluster_to_chain(&mut self, cluster: Cluster, new_cluster: Cluster) -> io::Result<()> {
        match self.fat_entry(new_cluster)?.status() {
            Status::Free => (),
            _ => return ioerr!(InvalidData, "new_cluster is not free"),
        }
        let mut current_cluster = cluster;
        loop {
            let fat_entry = self.fat_entry(current_cluster)?.status();
            match fat_entry {
                Status::Data(next_cluster) => {
                    current_cluster = next_cluster;
                },
                Status::Eoc(_) => {
                    let sector = current_cluster.fat_address() as u64 * 4 / self.bytes_per_sector as u64;
                    let position_in_sector = current_cluster
                        .fat_address() as usize * 4 - sector as usize * self.bytes_per_sector as usize;

                    if sector > self.sectors_per_fat as u64 {
                        return ioerr!(NotFound, "invalid cluster index: sector: add_cluster_to_chain")
                    }
                    let data = self
                        .device.get_mut(self.fat_start_sector + sector)?;
                    let new_cluster_fat_array = new_cluster.fat_address().to_le_bytes();
                    let mut data_slice = &mut data[position_in_sector..position_in_sector + 4];
                    let _bytes_written = data_slice.write(&new_cluster_fat_array)?;
                    // for index in 0..4 {
                    //     data[position_in_sector + index] = byte_array[index];
                    // }

                    let new_sector = new_cluster.fat_address() as u64 * 4 / self.bytes_per_sector as u64;
                    let position_in_new_sector = new_cluster
                        .fat_address() as usize * 4 - new_sector as usize * self.bytes_per_sector as usize;

                    if new_sector > self.sectors_per_fat as u64 {
                        return ioerr!(NotFound, "invalid cluster index: new sector: add_cluster_to_chain")
                    }
                    let new_data = self
                        .device.get_mut(self.fat_start_sector + new_sector)?;
                    let eoc_array = 0x0FFFFFFFu32.to_le_bytes();
                    let mut new_data_slice = &mut new_data[position_in_new_sector..position_in_new_sector + 4];
                    let _bytes_written = new_data_slice.write(&eoc_array)?;


                    // for index in 0..4 {
                    //     new_data[position_in_new_sector + index] = new_byte_array[index];
                    // }
                    return Ok(())
                },
                Status::Bad => return ioerr!(InvalidData, "cluster in chain marked bad"),
                Status::Reserved => return ioerr!(InvalidData, "cluster in chain marked reserved"),
                Status::Free => return ioerr!(InvalidData, "cluster in chain marked free"),
            };
        }

    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::vfat::File<HANDLE>;
    type Dir = crate::vfat::Dir<HANDLE>;
    type Entry = crate::vfat::Entry<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        use crate::traits::Entry as EntryTrait;

        let path = path.as_ref();
        if !path.is_absolute() {
            return ioerr!(InvalidInput, "path is not absolute")
        }

        let first_cluster = self.lock(|vfat| vfat.rootdir_cluster);
        let mut entry = Entry::Dir(Dir {
            vfat: self.clone(),
            first_cluster,
            name: String::from(""),
            metadata: Metadata::default(),
            size: 0,
            path: PathBuf::from("/"),
            parent_path: None,
            parent_first_cluster: None,
        });
        for component in path.components() {
            match component {
                // Component::ParentDir => {
                //     entry = entry.into_dir()
                //         .ok_or(newioerr!(InvalidInput, "path parent is not dir"))?
                //         .find("..")?;
                // },
                Component::Normal(name) => {
                    entry = entry.into_dir()
                        .ok_or(newioerr!(NotFound, "path not found"))?
                        .find(name)?;
                }
                _ => (),
            }
        }
        Ok(entry)
    }

    // fn set_metadata<P: AsRef<Path>>(self, path: P, metadata: Metadata) -> io::Result<()> {
    //     use crate::traits::Entry as EntryTrait;
    //
    //     let path = path.as_ref();
    //     if !path.is_absolute() {
    //         return ioerr!(InvalidInput, "path is not absolute")
    //     }
    //
    //     let first_cluster = self.lock(|vfat| vfat.rootdir_cluster);
    //     let mut entry = Entry::Dir(Dir {
    //         vfat: self.clone(),
    //         first_cluster,
    //         name: String::from(""),
    //         metadata: Metadata::default(),
    //         size: 0,
    //         path: PathBuf::from("/")
    //     });
    //     let mut found_entry: Entry<HANDLE>;
    //     for component in path.components() {
    //         match component {
    //             // Component::ParentDir => {
    //             //     entry = entry.into_dir()
    //             //         .ok_or(newioerr!(InvalidInput, "path parent is not dir"))?
    //             //         .find("..")?;
    //             // },
    //             Component::Normal(name) => {
    //                 found_entry = entry.into_dir()
    //                     .ok_or(newioerr!(NotFound, "path not found"))?
    //                     .find(name)?;
    //             }
    //             _ => (),
    //         }
    //     }
    //
    //     unimplemented!("VFat::set_metadata()")
    // }

}
