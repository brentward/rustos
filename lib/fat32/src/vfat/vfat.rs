use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::Path;

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

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
        if !partition_entry.is_vfat() {
            return Err(Error::NotFound)
        }
        let start_sector = partition_entry.start_sector() as u64;
        let ebpb = BiosParameterBlock::from(&mut device, start_sector)?;

        let partition = Partition {
            start: partition_entry.start_sector() as u64,
            num_sectors: partition_entry.total_sectors() as u64,
            sector_size: ebpb.sector_size()
        };

        let cached_partition = CachedPartition::new(device, partition);
        let bytes_per_sector = ebpb.sector_size() as u16;
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

    // TODO: The following methods may be useful here:
    //
    //  * A method to read from an offset of a cluster into a buffer.
    //
    fn read_cluster(
        &mut self,
        cluster: Cluster,
        offset: usize,
        buf: &mut [u8]
    ) -> io::Result<usize> {
        // let fat_entry = self.fat_entry(cluster)?;
        let first_sector = self.data_start_sector + cluster.value() as u64 * self.sectors_per_cluster as u64;
        let last_sector = first_sector + self.sectors_per_cluster as u64;
        let start_sector =  first_sector + offset as u64 / self.bytes_per_sector as u64;
        if offset_sectors > self.sectors_per_cluster as u64 {
            Err(io::Error::new(io::ErrorKind::Other, "offset larger than cluster"))
        }
        let sector_offset = offset % self.bytes_per_sector as usize;
        let data = self.device.get(start_sector)?;
        let mut bytes =buf.write(&data[sector_offset..])?;
        for sector in start_sector + 1..last_sector {
            let data = self.device.get(sector)?;
            bytes += buf.write(data)?;
        }
        Ok(bytes)


        // unimplemented!("Vfat::read_cluster")
    }
    //
    //  * A method to read all of the clusters chained from a starting cluster
    //    into a vector.
    //
    // fn read_chain(
    //     &mut self,
    //     start: Cluster,
    //     buf: &mut Vec<u8>
    // ) -> io::Result<usize> {
    //     mbr = MasterBootRecord::from(&self.device);
    //     self.device.read_all_sector(
    //         cluster as u64 + offset as u64 * self.device.sector_size(),
    //         buf
    //     )?
    // }
    //
    //  * A method to return a reference to a `FatEntry` for a cluster where the
    //    reference points directly into a cached sector.
    //
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let sector = cluster.value() as u64 * 4 / self.bytes_per_sector as u64;
        let position_in_sector = cluster.value() as usize * 4 - (sector as usize * self.bytes_per_sector as usize);
        let data = self.device.get(self.fat_start_sector + sector)?;
        Ok(unsafe { &data[position_in_sector..position_in_sector + 4].cast()[0] })
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::traits::Dummy;
    type Dir = crate::traits::Dummy;
    type Entry = crate::traits::Dummy;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        unimplemented!("FileSystem::open()")
    }
}
