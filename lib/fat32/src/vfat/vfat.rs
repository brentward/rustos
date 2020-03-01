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
    // fn read_cluster(
    //     &mut self,
    //     cluster: Cluster,
    //     offset: usize,
    //     buf: &mut [u8]
    // ) -> io::Result<usize> {
    //     self.device.read_sector(
    //         cluster as u64 + offset as u64 * self.device.sector_size(),
    //         buf
    //     )?
    // }
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
    // fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
    //
    // }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::traits::Dummy;
    type Dir = crate::traits::Dummy;
    type Entry = crate::traits::Dummy;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        unimplemented!("FileSystem::open()")
    }
}
