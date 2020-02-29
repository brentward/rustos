use core::fmt;
use shim::const_assert_size;
use core::mem;

use crate::traits::BlockDevice;
use crate::vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    jump_instruction: [u8; 3],
    oem_id: [u8; 8],
    bytes_per_sector: [u8; 2],
    sectors_per_cluster: u8,
    reserved_sectors: [u8; 2],
    fat_table_count: u8,
    max_directories: [u8; 2],
    logical_sectors: [u8; 2],
    media_descriptor: u8,
    sectors_per_fat: [u8; 2],
    sectors_per_track: [u8; 2],
    heads_count: [u8; 2],
    hidden_sector_count: [u8; 4],
    total_logical_sectors: [u8; 4],
    extended_sectors_per_fat: [u8; 4],
    flags: [u8; 2],
    fat_version: [u8; 2],
    root_cluster: [u8; 4],
    fsinfo_sector: [u8; 2],
    backup_boot_sector: [u8; 2],
    reserved: [u8; 12],
    driver_number: u8,
    windows_nt_flags: u8,
    signature: u8,
    volume_id: [u8; 4],
    volume_label: [u8; 11],
    system_identifier: [u8; 8],
    boot_code: [u8; 420],
    bootable_partition_signature: [u8; 2],
}

// const_assert_size!(BiosParameterBlock, 512);

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
        if !(bpb.signature == 0x28 || bpb.signature == 0x29) {
            return Err(Error::BadSignature);
        }
        Ok(bpb)
    }
}

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BiosParameterBlock")
            .field("jump instruction", &self.jump_instruction)
            .finish()
    }
}
