use alloc::string::String;

use shim::io::{self, SeekFrom};
use shim::ioerr;

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle, Status};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: usize,
    pub offset: usize,
    pub current_cluster: Option<Cluster>,
    pub bytes_per_cluster: usize,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    pub fn from(
        vfat: HANDLE,
        first_cluster: Cluster,
        name: String,
        metadata: Metadata,
        size: usize,
    ) -> File<HANDLE> {
        let bytes_per_cluster = vfat.lock(
            |vfat| vfat.bytes_per_cluster()
        );
        File {
            vfat,
            first_cluster,
            name,
            metadata,
            size,
            offset: 0,
            current_cluster: Some(first_cluster),
            bytes_per_cluster,
        }
    }
}

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
                    return ioerr!(InvalidInput, "beyond end of file")
                } else {
                    self.offset = offset as usize;
                }
            }
            SeekFrom::End(offset) => {
                if self.size() as i64 + offset < 0 {
                    return ioerr!(InvalidInput, "beyond beginning of file")
                } else {
                    self.offset = (self.size() as i64 + offset) as usize;
                }
            }
            SeekFrom::Current(offset) => {
                if self.offset as i64 + offset < 0 {
                    return ioerr!(InvalidInput, "beyond beginning of file")
                } else if self.offset as i64 + offset > self.size() as i64 {
                    return ioerr!(InvalidInput, "beyond end of file")
                } else {
                    self.offset = (self.offset as i64 + offset) as usize;
                }
            }
        }
        let mut current_cluster = self.first_cluster;
        let return_result = self.vfat.lock(|vfat| {
            let mut result = Ok(self.offset as u64);
            for _ in 0..(self.offset / self.bytes_per_cluster) {
                let fat_entry = vfat.fat_entry(current_cluster)?;
                result = match fat_entry.status() {
                    Status::Data(next_cluster) => {
                        current_cluster = next_cluster;
                        Ok(self.offset as u64)
                    },
                    _ => ioerr!(InvalidData, "Unexpected invalid cluster in chain"),
                };
            }
            result
        });
        match return_result {
            Ok(result) => {
                self.current_cluster = Some(current_cluster);
                Ok(result)
            },
            Err(e) => Err(e),
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
        let mut bytes_read = 0usize;
        let max_bytes = (self.size as usize  - self.offset as usize)
            .min(buf.len());
        let mut current_cluster_result = Ok(self.current_cluster);
        let mut current_cluster = current_cluster_result?;
        let mut cluster_offset = self.offset % self.bytes_per_cluster;
        while bytes_read < max_bytes {
            let bytes = self.vfat.lock(
                |vfat| {
                    vfat.read_cluster(
                        current_cluster.unwrap(),
                        cluster_offset,
                        &mut buf[bytes_read..max_bytes]
                    )
                }
            )?;
            if bytes == self.bytes_per_cluster - cluster_offset {
                current_cluster_result = self.vfat.lock(|vfat| {
                    match vfat.fat_entry(current_cluster.unwrap())?.status() {
                        Status::Data(cluster) => Ok(Some(cluster)),
                        Status::Eoc(_) => Ok(None),
                        Status::Bad => ioerr!(InvalidInput, "cluster in chain marked bad"),
                        Status::Reserved => {
                            ioerr!(InvalidInput, "cluster in chain marked reserved")
                        }
                        Status::Free => ioerr!(InvalidInput, "cluster in chain marked free"),
                    }
                });
                current_cluster = current_cluster_result?;

            }
            bytes_read += bytes;
            cluster_offset = 0;
        }
        self.current_cluster = current_cluster;
        self.offset += max_bytes;
        Ok(bytes_read)
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
