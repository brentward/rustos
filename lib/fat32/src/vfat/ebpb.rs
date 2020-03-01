use core::fmt;
use shim::const_assert_size;
use core::mem;

use crate::traits::BlockDevice;
use crate::vfat::Error;

struct Word {
    minor: u8,
    major: u8
}

impl Word {
    fn value(&self) -> u16 {
        ((self.major as u16) << 8) + self.minor as u16
    }

}

impl fmt::Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Word")
            .field("value", &self.value())
            .finish()
    }
}

#[repr(C, packed)]
pub struct BiosParameterBlock {
    jump_instruction: [u8; 3],
    oem_id: [u8; 8],
    bytes_per_sector: Word,
    sectors_per_cluster: u8,
    reserved_sectors: Word,
    fat_table_count: u8,
    max_directories: Word,
    logical_sectors: Word,
    media_descriptor: u8,
    sectors_per_fat: Word,
    sectors_per_track: Word,
    heads_count: Word,
    hidden_sector_count: u32,
    total_logical_sectors: u32,
    sectors_per_fat32: u32,
    flags: Word,
    fat_version: Word,
    root_dir_cluster: u32,
    fsinfo_sector: Word,
    backup_boot_sector: Word,
    reserved: [u8; 12],
    drive_number: u8,
    windows_nt_flags: u8,
    signature: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    system_identifier: [u8; 8],
    boot_code: [u8; 420],
    bootable_partition_signature: [u8; 2],
}

const_assert_size!(BiosParameterBlock, 512);

impl BiosParameterBlock {
    /// Reads the FAT32 extended BIOS parameter block from sector `sector` of
    /// device `device`.
    ///
    /// # Errors
    ///
    /// If the EBPB signature is invalid, returns an error of `BadSignature`.
    pub fn from<T: BlockDevice>(mut device: T, sector: u64) -> Result<BiosParameterBlock, Error> {
        let mut buf = [0u8; 512];
        match device.read_sector(sector, &mut buf) {
            Err(err) => return Err(Error::Io(err)),
            _ => (),
        }
        let bpb: BiosParameterBlock = unsafe { mem::transmute(buf) };
        if bpb.bootable_partition_signature != [0x55, 0xAA] {
            return Err(Error::BadSignature);
        }
        Ok(bpb)
    }

    pub fn bytes_per_sector(&self) -> u64 {
        self.bytes_per_sector.value() as u64
    }

    pub fn sectors_per_cluster(&self) -> u8 {
        self.sectors_per_cluster
    }

    pub fn sectors_per_fat(&self) -> u32 {
        self.sectors_per_fat32
    }

    pub fn fat_start_sector(&self) -> u64 {
        self.reserved_sectors.value() as u64
    }

    pub fn data_start_sector(&self) -> u64 {
        self.fat_start_sector() + self.fat_table_count as u64 * self.sectors_per_fat() as u64
    }

    pub fn root_dir_cluster(&self) -> u32 {
        self.root_dir_cluster
    }
 }

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BiosParameterBlock")
            .field("jump_instruction", &self.jump_instruction)
            .field("oem_id", &self.oem_id)
            .field("bytes_per_sector", &self.bytes_per_sector)
            .field("sectors_per_cluster", &self.sectors_per_cluster)
            .field("reserved_sectors", &self.reserved_sectors)
            .field("fat_table_count", &self.fat_table_count)
            .field("max_directories", &self.max_directories)
            .field("logical_sectors", &self.logical_sectors)
            .field("media_descriptor", &self.media_descriptor)
            .field("sectors_per_fat", &self.sectors_per_fat)
            .field("sectors_per_track", &self.sectors_per_track)
            .field("heads_count", &self.heads_count)
            .field("hidden_sector_count", &self.hidden_sector_count)
            .field("total_logical_sectors", &self.total_logical_sectors)
            .field("sectors_per_fat32", &self.sectors_per_fat32)
            .field("flags", &self.flags)
            .field("fat_version", &self.fat_version)
            .field("root_cluster", &self.root_dir_cluster)
            .field("fsinfo_sector", &self.fsinfo_sector)
            .field("backup_boot_sector", &self.backup_boot_sector)
            .field("reserved", &self.reserved)
            .field("drive_number", &self.drive_number)
            .field("windows_nt_flags", &self.windows_nt_flags)
            .field("signature", &self.signature)
            .field("volume_id", &self.volume_id)
            .field("volume_label", &self.volume_label)
            .field("system_identifier", &self.system_identifier)
            .field("bootable_partition_signature", &self.bootable_partition_signature)
            .finish()
    }
}
