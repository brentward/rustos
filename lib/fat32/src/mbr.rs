use core::fmt;
use shim::const_assert_size;
use shim::io;

use core::mem;

use crate::traits::BlockDevice;

#[derive(Copy, Clone)]
struct SectorCylinder([u8; 2]);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHS {
    head: u8,
    sector_cylinder: SectorCylinder,
    }

impl CHS {
    fn sector(&self) -> u16 {
        let sector_cylinder: u16 =
            ((self.sector_cylinder.0[0] as u16) << 8) & self.sector_cylinder.0[1] as u16;
        sector_cylinder & !(0b1111111111 << 6)
    }

    fn cylinder(&self) -> u16 {
        let sector_cylinder: u16 =
            ((self.sector_cylinder.0[0] as u16) << 8) & self.sector_cylinder.0[1] as u16;
        sector_cylinder >> 6
    }
}

impl fmt::Debug for CHS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CHS")
            .field("head", &self.head)
            .field("sector", &self.sector())
            .field("cylinder", &self.cylinder())
            .finish()
    }
}

const_assert_size!(CHS, 3);

#[repr(C, packed)]
pub struct PartitionEntry {
    boot_flag: u8,
    start_chs: CHS,
    partition_type: u8,
    end_chs: CHS,
    start_sector: u32,
    total_sectors: u32,
}

impl PartitionEntry {
    pub fn bootable(&self) -> bool {
        self.boot_flag == 0x80
    }

    pub fn is_fat32(&self) -> bool {
        (self.partition_type == 0xB || self.partition_type == 0xC)
    }

    pub fn start_sector(&self) -> u32 {
        self.start_sector
    }

    pub fn total_sectors(&self) -> u32 {
        self.total_sectors
    }
}

impl fmt::Debug for PartitionEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PartitionEntry")
            .field("boot flag", &self.boot_flag)
            .field("start CHS", &self.start_chs)
            .field("partition type", &self.partition_type)
            .field("end CHS", &self.end_chs)
            .field("start sector", &self.start_sector)
            .field("total sector", &self.total_sectors)
            .finish()
    }
}

const_assert_size!(PartitionEntry, 16);

struct Bootstrap([u8; 436]);

#[derive(Debug)]
struct DiskID([u8; 10]);

/// The master boot record (MBR).
#[repr(C, packed)]
pub struct MasterBootRecord {
    // FIXME: Fill me in.
    bootstrap: Bootstrap,
    disk_id: DiskID,
    partitions: [PartitionEntry; 4],
    magic_signature: [u8; 2],

}

impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MasterBootRecord")
            .field("disk id", &self.disk_id)
            .field("partition 1", &self.partitions[0])
            .field("partition 2", &self.partitions[1])
            .field("partition 3", &self.partitions[2])
            .field("partition 4", &self.partitions[3])
            .field("magic signature", &self.magic_signature)
            .finish()
    }
}
const_assert_size!(MasterBootRecord, 512);

#[derive(Debug)]
pub enum Error {
    /// There was an I/O error while reading the MBR.
    Io(io::Error),
    /// Partiion `.0` (0-indexed) contains an invalid or unknown boot indicator.
    UnknownBootIndicator(u8),
    /// The MBR magic signature was invalid.
    BadSignature,
}

impl MasterBootRecord {
    /// Reads and returns the master boot record (MBR) from `device`.
    ///
    /// # Errors
    ///
    /// Returns `BadSignature` if the MBR contains an invalid magic signature.
    /// Returns `UnknownBootIndicator(n)` if partition `n` contains an invalid
    /// boot indicator. Returns `Io(err)` if the I/O error `err` occured while
    /// reading the MBR.
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        // let sector_size = device.sector_size();
        let mut buf = [0u8; 512];
        match device.read_sector(0, &mut buf) {
            Err(err) => return Err(Error::Io(err)),
            _ => (),
        }
        let mbr: MasterBootRecord = unsafe { mem::transmute(buf) };
        if mbr.magic_signature != [0x55u8, 0xAAu8] {
            return Err(Error::BadSignature);
        }
        for (index, partition) in mbr.partitions.iter().enumerate() {
            if !(partition.boot_flag == 0 || partition.boot_flag == 0x80) {
                return Err(Error::UnknownBootIndicator(index as u8));
            }
        }
        Ok(mbr)
    }

    pub fn get_partition(&self, number: usize) -> &PartitionEntry {
        let partition = &self.partitions[number];
        partition
    }
}
