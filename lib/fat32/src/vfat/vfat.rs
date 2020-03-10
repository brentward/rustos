use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;
use alloc::string::String;

use shim::io::{self, Write};
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::{Path, Component};

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
    rootdir_cluster: Cluster,
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
            bytes += bytes_written; // TODO Go back to old method in loop and remove mut from buf
        }
        Ok(bytes)
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

    pub fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let sector = cluster.fat_address() as u64 * 4 / self.bytes_per_sector as u64;
        let position_in_sector = cluster
            .fat_address() as usize * 4 - sector as usize * self.bytes_per_sector as usize;
        if sector > self.sectors_per_fat as u64 {
            return ioerr!(NotFound, "invalid cluster index")
        }
        let data = self
            .device.get(self.fat_start_sector + sector)?;
        Ok(unsafe { &data[position_in_sector..position_in_sector + 4].cast()[0] })
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
        let mut dir = Entry::Dir(Dir {
            vfat: self.clone(),
            first_cluster,
            name: String::from(""),
            metadata: Metadata::default(),
            size: 0,
        });
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    dir = dir.into_dir()
                        .ok_or(newioerr!(InvalidInput, "path parent is not dir"))?
                        .find("..")?;
                },
                Component::Normal(name) => {
                    dir = dir.into_dir()
                        .ok_or(newioerr!(NotFound, "path not found"))?
                        .find(name)?;
                }
                _ => (),
            }
        }
        Ok(dir)
    }
}
