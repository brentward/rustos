use alloc::string::String;

use shim::io::{self, SeekFrom};
use shim::ioerr;

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle, Status};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub first_cluster: Cluster,
    // pub current_sector_location: (Cluster, usize),
    pub name: String,
    pub metadata: Metadata,
    pub size: usize,
    pub offset: usize,
    pub current_cluster: Option<Cluster>,
    pub bytes_per_cluster: usize,
    // current_cluster_offset: usize,
    // pub bytes_per_cluster: u32,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    // pub fn offset(&self) -> usize {
    //     self.offset
    // }
    //
    // pub fn current_cluster(&self) -> Option<Cluster> {
    //     self.current_cluster
    // }
    //
    // pub fn current_cluster_offset(&self) -> usize {
    //     self.current_cluster_offset
    // }
    //
    // pub fn add_offset(&mut self, add_offset: usize) -> io::Result<()> {
    //     self.set_offset(self.offset() + add_offset)
    // }
    //
    // pub fn set_offset(&mut self, offset: usize) -> io::Result<(())> {
    //     if offset >= self.size {
    //         return Err(io::Error::new(
    //             io::ErrorKind::UnexpectedEof,
    //             "attempt to set position beyond end of file"
    //         ))
    //     } else {
    //         let bytes_per_cluster = self.vfat
    //             .lock(|vfat| vfat.bytes_per_cluster());
    //         let cluster_index = self.offset / bytes_per_cluster;
    //         // let mut current_cluster_result = Ok(self.current_cluster.unwrap()?);
    //         let mut current_cluster = self.current_cluster();
    //         for i in 0..cluster_index {
    //             self.vfat.lock(|vfat| {
    //                 // let mut current_cluster = current_cluster_result?;
    //                     // let fat_entry = vfat.fat_entry(self.current_cluster())?;
    //                     match vfat.fat_entry(current_cluster.unwrap())?.status() {
    //                             Status::Data(cluster) => {
    //                                 current_cluster = Some(cluster);
    //                                 Ok(current_cluster)
    //                             },
    //                             Status::Eoc(_) => {
    //                                 // if i + 1 != cluster_index {
    //                                 //     return Err(io::Error::new(
    //                                 //         io::ErrorKind::UnexpectedEof,
    //                                 //         "file ended unexpectedly early"
    //                                 //     ))
    //                                 // };
    //                                 current_cluster = None;
    //                                 Ok(current_cluster)
    //                             }
    //                                 // Cluster::from(0xFFFFFFFF)
    //                             Status::Bad => Err(io::Error::new(
    //                                 io::ErrorKind::InvalidData,
    //                                 "cluster in chain unexpectedly marked bad"
    //                             )),
    //                             Status::Reserved => Err(io::Error::new(
    //                                 io::ErrorKind::InvalidData,
    //                                 "cluster in chain unexpectedly marked reserved"
    //                             )),
    //                             Status::Free => Err(io::Error::new(
    //                                 io::ErrorKind::InvalidData,
    //                                 "cluster in chain unexpectedly marked free"
    //                             )),
    //                     }
    //                 });
    //             // self.current_cluster = match fat_entry.status() {
    //             //     Status::Data(cluster) => cluster,
    //             //     Status::Eoc(_cluster_address) => {
    //             //         // if i + 1 != cluster_index {
    //             //         //     return Err(io::Error::new(
    //             //         //         io::ErrorKind::UnexpectedEof,
    //             //         //         "file ended unexpectedly early"
    //             //         //     ))
    //             //         // };
    //             //         Cluster::from(_cluster_address)
    //             //         // Cluster::from(0xFFFFFFFF)
    //             //     },
    //             //     Status::Bad => return Err(io::Error::new(
    //             //         io::ErrorKind::InvalidData,
    //             //         "cluster in chain unexpectedly marked bad"
    //             //     )),
    //             //     Status::Reserved => return Err(io::Error::new(
    //             //         io::ErrorKind::InvalidData,
    //             //         "cluster in chain unexpectedly marked reserved"
    //             //     )),
    //             //     Status::Free => return Err(io::Error::new(
    //             //         io::ErrorKind::InvalidData,
    //             //         "cluster in chain unexpectedly marked free"
    //             //     )),
    //             // };
    //         }
    //         // match current_cluster_result {
    //         //     Ok(cluster) => {
    //         //         self.offset = offset;
    //         //         self.current_cluster = cluster;
    //         //         self.current_cluster_offset = (self.offset % bytes_per_cluster);
    //         //         Ok(())
    //         //     }
    //         //     Err(e) => Err(e),
    //         // }
    //         self.offset = offset;
    //         self.current_cluster = current_cluster;
    //         self.current_cluster_offset = (self.offset % bytes_per_cluster);
    //
    //         // match current_cluster {
    //         //     Some(cluster) => {
    //         //         self.current_cluster = cluster;
    //         //         Ok(())
    //         //     }
    //         //     None => Err(io::Error::new(
    //         //         io::ErrorKind::InvalidData,
    //         //         "Invalid cluster in file chain"
    //         //     ))
    //         //
    //         // }
    //         Ok(())
    //     }
    //
    // }
    // pub fn current_cluster_and_offset(&self) -> io::Result<(Cluster, usize)> {
    //     if self.offset >= self.size {
    //         panic!("File::current_cluster_and_offset() called when at or past EOF")
    //     } else {
    //         let bytes_per_cluster = self.vfat
    //             .lock(|vfat| vfat.bytes_per_cluster());
    //         let current_cluster_offset = (self.offset % bytes_per_cluster);
    //         let cluster_index = self.offset / bytes_per_cluster;
    //         let mut current_cluster = self.first_cluster;
    //         for i in 0..cluster_index {
    //             let fat_entry = self.vfat
    //                 .lock(|vfat| &vfat.fat_entry(current_cluster))?;
    //             match fat_entry.status() {
    //                 Status::Data(cluster) => cluster,
    //                 Status::Eoc(_cluster_address) => {
    //                     // if i + 1 != cluster_index {
    //                     //     return Err(io::Error::new(
    //                     //         io::ErrorKind::UnexpectedEof,
    //                     //         "file ended unexpectedly early"
    //                     //     ))
    //                     // };
    //                     // Cluster::from(next_cluster)
    //                     Cluster::from(0xFFFFFFFF)
    //                 },
    //                 Status::Bad => return Err(io::Error::new(
    //                     io::ErrorKind::InvalidData,
    //                     "cluster in chain unexpectedly marked bad"
    //                 )),
    //                 Status::Reserved => return Err(io::Error::new(
    //                     io::ErrorKind::InvalidData,
    //                     "cluster in chain unexpectedly marked reserved"
    //                 )),
    //                 Status::Free => return Err(io::Error::new(
    //                     io::ErrorKind::InvalidData,
    //                     "cluster in chain unexpectedly marked free"
    //                 )),
    //             };
    //         }
    //
    //
    //         Ok((current_cluster, current_cluster_offset))
    //     }
    //     // unimplemented!("working on it")
    // }
    pub fn from(
        vfat: HANDLE,
        first_cluster: Cluster,
        name: String,
        metadata: Metadata,
        size: usize,
    ) -> File<HANDLE> {
        let bytes_per_cluster = vfat.lock(|vfat| vfat.bytes_per_cluster());
        File {
            vfat,
            first_cluster,
            // current_sector_location: (Cluster::from(regular_dir.cluster()), 0),
            name,
            metadata,
            size,
            offset: 0,
            current_cluster: Some(first_cluster),
            bytes_per_cluster,
            // current_cluster_offset: 0
        }
    }
}

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
                    self.offset = offset as usize;
                    // self.set_offset(offset as usize)?;
                    // self.current_sector_location = self.vfat.lock(
                    //     |a| {
                    //         a.current_sector(
                    //             self.current_sector_location.0,
                    //             self.current_offset - self.current_sector_location.1
                    //         )
                    //     }
                    // )?;
                    Ok(offset)
                }
            }
            SeekFrom::End(offset) => {
                if self.size() as i64 + offset < 0 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond beginning of file"))
                } else {
                    self.offset = (self.size() as i64 + offset) as usize;
                    // self.current_sector_location = self.vfat.lock(
                    //     |a| {
                    //         a.current_sector(
                    //             self.current_sector_location.0,
                    //             self.current_offset - self.current_sector_location.1
                    //         )
                    //     }
                    // )?;
                    Ok(self.offset as u64)
                }
            }
            SeekFrom::Current(offset) => {
                if self.offset as i64 + offset < 0 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond beginning of file"))
                } else if self.offset as i64 + offset > self.size() as i64 {
                    Err(io::Error::new(io::ErrorKind::InvalidInput, "Beyond end of file"))
                } else {
                    self.offset = (self.offset as i64 + offset) as usize;
                    // self.current_sector_location = self.vfat.lock(
                    //     |a| {
                    //         a.current_sector(
                    //             self.current_sector_location.0,
                    //             self.current_offset - self.current_sector_location.1
                    //         )
                    //     }
                    // )?;
                    Ok(self.offset as u64)
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
        let mut bytes_read = 0usize;
        let max_bytes = (self.size as usize  - self.offset as usize)
            .min(buf.len());
        let mut current_cluster_result = Ok(self.current_cluster);
        let mut current_cluster = current_cluster_result?;
        let mut cluster_offset = self.offset % self.bytes_per_cluster;
        while bytes_read < max_bytes {
            // let (current_cluster, current_cluster_offset) = self.current_cluster_and_offset()?;
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
                    // let mut current_cluster = current_cluster_result?;
                    // let fat_entry = vfat.fat_entry(self.current_cluster())?;
                    match vfat.fat_entry(current_cluster.unwrap())?.status() {
                        Status::Data(cluster) => Ok(Some(cluster)),// {
                        //     current_cluster = Some(cluster);
                        //     Ok(current_cluster)
                        // },
                        Status::Eoc(_) => Ok(None), //{
                            // if i + 1 != cluster_index {
                            //     return Err(io::Error::new(
                            //         io::ErrorKind::UnexpectedEof,
                            //         "file ended unexpectedly early"
                            //     ))
                            // };
                            // current_cluster = None;
                            // Ok(current_cluster)
                        // }
                        // Cluster::from(0xFFFFFFFF)
                        Status::Bad => Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "cluster in chain unexpectedly marked bad"
                        )),
                        Status::Reserved => Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "cluster in chain unexpectedly marked reserved"
                        )),
                        Status::Free => Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "cluster in chain unexpectedly marked free"
                        )),
                    }
                });
                current_cluster = current_cluster_result?;

            }

            // if bytes == 0 {
            //     // Problem in this method with resuming read in position
            //     break
            // }
            bytes_read += bytes;
            cluster_offset = 0;
            // self.add_offset(bytes);
            // self.current_sector_location = self.vfat.lock(
            //     |a| {
            //         a.current_sector(
            //             self.current_sector_location.0,
            //             self.current_offset - self.current_sector_location.1
            //         )
            //     }
            // )?;
        }
        self.current_cluster = current_cluster;
        self.offset += max_bytes;
        Ok(bytes_read)
        // let mut full_buf = Vec::new();
        // let bytes = self.vfat.lock(|a| a.read_chain(self.first_cluster, &mut full_buf))?;
        // let max_size = buf.len().min(bytes - self.current_position);
        // while self.current_position > self.size() as usize
        // for index in 0..max_size {
        //     buf[index] = full_buf[index + self.current_position];
        // }
        // self.current_position += max_size;
        // Ok(max_size)
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
