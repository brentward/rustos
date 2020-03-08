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

    pub fn current_sector(&mut self, start: Cluster, offset: usize) -> io::Result<(Cluster, usize)> {
        // TODO Remove VFat::current_sector()
        unimplemented!("remove this method: VFat::current_sector()")
    }
    //     let cluster_size = self.bytes_per_sector as usize * self.sectors_per_cluster as usize;
    //     let cluster_index = offset / cluster_size;
    //     let mut current_cluster = start;
    //
    //     for i in 0..cluster_index {
    //         let fat_entry = self.fat_entry(current_cluster)?.status();
    //          current_cluster = match fat_entry {
    //             Status::Data(next_cluster) => next_cluster,
    //             Status::Eoc(next_cluster) => {
    //                 // if i + 1 != cluster_index {
    //                 //     return Err(io::Error::new(
    //                 //         io::ErrorKind::UnexpectedEof,
    //                 //         "file ended unexpectedly early"
    //                 //     ))
    //                 // };
    //                 Cluster::from(next_cluster)
    //                 // Cluster::from(0xFFFFFFFF)
    //             },
    //             Status::Bad => return Err(io::Error::new(
    //                 io::ErrorKind::InvalidData,
    //                 "cluster in chain unexpectedly marked bad"
    //             )),
    //             Status::Reserved => return Err(io::Error::new(
    //                 io::ErrorKind::InvalidData,
    //                 "cluster in chain unexpectedly marked reserved"
    //             )),
    //             Status::Free => return Err(io::Error::new(
    //                 io::ErrorKind::InvalidData,
    //                 "cluster in chain unexpectedly marked free"
    //             )),
    //         };
    //     }
    //
    //     Ok((current_cluster, cluster_index * cluster_size))
    // }

    // TODO: The following methods may be useful here:
    //
    //  * A method to read from an offset of a cluster into a buffer.
    //
    pub fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        mut buf: &mut [u8]
    ) -> io::Result<usize> {
        // let fat_entry = self.fat_entry(cluster)?;
        let first_sector = self.data_start_sector + (cluster.data_address() as u64 * self.sectors_per_cluster as u64);
        let mut sector_index = offset as u64 / self.bytes_per_sector as u64 ;
        let mut bytes = 0usize;
        loop {
            sector_index = (offset + bytes) as u64 / self.bytes_per_sector as u64;
            if sector_index >= self.sectors_per_cluster as u64 {
                break;
            }
            let current_offset = (offset + bytes) - (sector_index as usize * self.bytes_per_sector as usize);
            let data = self.device.get(first_sector + sector_index)?;
            let bytes_written = buf.write(&data[current_offset..])?;
            bytes += bytes_written; // TODO Go back to old method in loop and remove mut from buf
            if buf.is_empty() {
                break;
            }

            // for &byte in data[current_offset..data.len().min(current_offset + buf.len())].iter() {
            //     buf[bytes] = byte;
            //     bytes += 1;
            //     if bytes == buf.len() {
            //         break 'fill_buf
            //     }
            // }


            // bytes += buf.write(&data[current_offset..])?;
        }

        // let start_sector =  first_sector + (offset as u64 / self.bytes_per_sector as u64);
        // if start_sector - first_sector > self.sectors_per_cluster as u64 {
        //     return Err(io::Error::new(io::ErrorKind::Other, "offset larger than cluster"))
        // };
        // let sector_offset = offset % self.bytes_per_sector as usize;
        // let data = self.device.get(start_sector)?;
        // let mut bytes = 0usize;
        // for byte in data[sector_offset..].iter() {
        //     buf[bytes] = *byte;
        //     bytes += 1;
        // }
        // for sector in start_sector + 1..last_sector {
        //     let data = self.device.get(sector)?;
        //     for byte in data.iter() {
        //         buf[bytes] = *byte;
        //         bytes += 1;
        //     }
        // }
        Ok(bytes)
    }
    //
    //  * A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    //
    fn add_cluster_to_buf(&mut self, cluster: Cluster, buf: &mut Vec<u8>) -> io::Result<usize> {
        let start = buf.len();

        let bytes_per_cluster = (self.sectors_per_cluster as u16 * self.bytes_per_sector) as usize;
        let max_size = start + bytes_per_cluster;
        buf.reserve(max_size);
        // unsafe {
        //     buf.set_len(start + bytes_per_cluster);
        // }
        // let bytes_read = self.read_cluster(cluster, 0, &mut buf[start..start + bytes_per_cluster])?;

        let mut cluster_chunk_buf = [0u8; 512];
        let mut bytes_total = 0usize;
        for cluster_chunk in 0usize..(bytes_per_cluster as usize / 512) {
            let bytes_read = self.read_cluster(cluster, cluster_chunk * 512, &mut cluster_chunk_buf)?;
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
        // let mut clusters = vec![start];
        // let mut fat_entry = self.fat_entry(start)?;
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
                //     Err(io::Error::new(
                //     io::ErrorKind::InvalidData,
                //     "cluster in chain unexpectedly marked bad"
                // )),
                Status::Reserved => return ioerr!(InvalidData, "cluster in chain marked reserved"),
                //     Err(io::Error::new(
                //     io::ErrorKind::InvalidData,
                //     "cluster in chain unexpectedly marked reserved"
                // )),
                Status::Free => return ioerr!(InvalidData, "cluster in chain marked free"),
                //     Err(io::Error::new(
                //     io::ErrorKind::InvalidData,
                //     "cluster in chain unexpectedly marked free"
                // )),
            };
            // clusters.push(next_cluster);
            // fat_entry = self.fat_entry(next_cluster)?;
        }
        // let mut bytes = 0usize;
        //
        // for cluster in clusters.iter() {
        //     let first_sector = self.data_start_sector + cluster.data_address() as u64 * self.sectors_per_cluster as u64;
        //     let last_sector = first_sector + self.sectors_per_cluster as u64;
        //     for sector in first_sector..last_sector {
        //         let data = self.device.get(sector)?;
        //         for byte in data.iter() {
        //             buf.push(*byte);
        //             bytes += 1;
        //         }
        //     }
        // }
    Ok(bytes)
    }
    //
    //  * A method to return a reference to a `FatEntry` for a cluster where the
    //    reference points directly into a cached sector.
    //
    pub fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let sector = cluster.fat_address() as u64 * 4 / self.bytes_per_sector as u64;
        let position_in_sector = cluster.fat_address() as usize * 4 - sector as usize * self.bytes_per_sector as usize;
        if sector > self.sectors_per_fat as u64 {
            return ioerr!(NotFound, "invalid cluster index")
                // Err(io::Error::new(io::ErrorKind::NotFound,
                //                       "Invalid cluster index"));
        }
        let data = self
            .device.get(self.fat_start_sector + sector)?;
        let zero = if data.len() == 0 {
            Some(())
        } else {
            None
        };
        Ok(unsafe { &data[position_in_sector..position_in_sector + 4].cast()[0] })
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::vfat::File<HANDLE>;
    type Dir = crate::vfat::Dir<HANDLE>;
    type Entry = crate::vfat::Entry<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        use crate::vfat::{Dir, File, Entry};
        use crate::traits::Entry as EntryTrait;
        use Component;

        let path = path.as_ref();
        if !path.is_absolute() {
            return ioerr!(InvalidInput, "path is not absolute")//Err(io::Error::new(io::ErrorKind::InvalidInput, "path is not absolute"))
        }

        let mut first_cluster = self.lock(|vfat| vfat.rootdir_cluster);
        // let metadata = Metadata::from((0, [0, 0, 0, 0, 0]));
        let mut dir = Entry::Dir(Dir {
            vfat: self.clone(),
            first_cluster,
            name: String::from(""),
            metadata: Metadata::default(),
            size: 0,
        });
        // let mut path_vec = Vec::new();
        // path_vec = path.components().collect();
        // if path_vec.len() == 1 {
        //     return found_result;
        // } else {
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    dir = dir.into_dir().ok_or(newioerr!(InvalidInput, "path parent is not dir"))?
                        .find("..")?;
                    // io::Error::new(io::ErrorKind::InvalidInput,
                    //                "Expected dir")
                },
                Component::Normal(name) => {
                    dir = dir.into_dir().ok_or(newioerr!(NotFound, "path not found"))?.find(name)?;
                    // io::Error::new(io::ErrorKind::NotFound,
                    //                "Expected dir")
                }
                _ => (),
            }
            // let found_entry = match found_result {
            //     Ok(entry) => entry,
            //     Err(e) => return Err(e),
            // };
            // let working_dir = match found_entry.into_dir() {
            //     Some(dir) => dir,
            //     None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is not valid")),
            // };
            // found_result = working_dir.find(component);
        }
        Ok(dir)
        // // }
        // match found_result {
        //     Ok(entry) => Ok(entry),
        //     Err(_) => Err(io::Error::new(io::ErrorKind::NotFound, "path is not found")),
        // }        // // Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond end of file"))
        // // unimplemented!("FileSystem::open()")
    }
}
