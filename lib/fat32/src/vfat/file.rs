use alloc::string::String;

use shim::io::{self, SeekFrom};

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: u32,
    pub chain_index: usize,
}

// impl<HANDLE: VFatHandle> File<HANDLE> {
//     pub fn from(
//         vfat: HANDLE,
//         first_cluster: Cluster,
//         name: String,
//         attributes: u8,
//         create_time: u16,
//         create_date: u16,
//         accessed_date: u16,
//         modification_time: u16,
//         nodification_date: u16,
//         size: u32,
//     ) -> File<HANDLE> {
//         let metadata = Metadata {
//
//         }
//         File {
//             v
//         }
//
//     }
// }

// FIXME: Implement `traits::File` (and its supertraits) for `File`.

impl<HANDLE: VFatHandle> io::Seek for File<HANDLE> {
    /// Seek to offset `pos` in the file.
    ///
    /// A seek to the end of the file is allowed. A seek _beyond_ the end of the
    /// file returns an `InvalidInput` error.
    ///
    /// If the seek operation completes successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with SeekFrom::Start.
    ///
    /// # Errors
    ///
    /// Seeking before the start of a file or beyond the end of the file results
    /// in an `InvalidInput` error.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        use crate::traits::File;
        match pos {
            SeekFrom::Start(offset) => {
                if offset > self.size() {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond end of file"))
                } else {
                    self.chain_index = offset as usize;
                    Ok(self.chain_index as u64)
                }
            }
            SeekFrom::End(offset) => {
                if self.size() as i64 + offset < 0 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond beginning of file"))
                } else {
                    self.chain_index = (self.size() as i64 + offset) as usize;
                    Ok(self.chain_index as u64)
                }
            }
            SeekFrom::Current(offset) => {
                if self.chain_index as i64 + offset < 0 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond beginning of file"))
                } else if self.chain_index as i64 + offset > self.size() as i64 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond end of file"))
                } else {
                    self.chain_index = (self.chain_index as i64 + offset) as usize;
                    Ok(self.chain_index as u64)
                }
            }
        }
    }
}

impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        unimplemented!("File::write()")
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<HANDLE: VFatHandle> io::Read for File<HANDLE> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut full_buf = Vec::new();
        let bytes = self.vfat.lock(|a| a.read_chain(self.first_cluster, &mut full_buf))?;
        let max_size = buf.len().min(bytes - self.chain_index);
        for index in 0..max_size {
            buf[index] = full_buf[index + self.chain_index];
        }
        self.chain_index += max_size;
        Ok(max_size)
    }
}

impl<HANDLE: VFatHandle> traits::File for File<HANDLE> {
    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn size(&self) -> u64 {
        self.size as u64
    }
}
