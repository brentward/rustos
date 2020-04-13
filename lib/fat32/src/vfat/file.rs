use alloc::string::String;
use alloc::vec::Vec;

use shim::io::{self, SeekFrom};
use shim::ioerr;
use shim::newioerr;
use shim::path::{Path, PathBuf, Component};

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle, Status, Dir, Entry};

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
    pub parent_path: PathBuf,
    pub parent_first_cluster: Cluster,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    pub fn from(
        vfat: HANDLE,
        first_cluster: Cluster,
        name: String,
        metadata: Metadata,
        size: usize,
        parent_path: PathBuf,
        parent_first_cluster: Cluster,
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
            parent_path,
            parent_first_cluster,
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
                    ioerr!(InvalidInput, "beyond end of file")
                } else {
                    self.offset = offset as usize;
                    Ok(offset)
                }
            }
            SeekFrom::End(offset) => {
                if self.size() as i64 + offset < 0 {
                    ioerr!(InvalidInput, "beyond beginning of file")
                } else {
                    self.offset = (self.size() as i64 + offset) as usize;
                    Ok(self.offset as u64)
                }
            }
            SeekFrom::Current(offset) => {
                if self.offset as i64 + offset < 0 {
                    ioerr!(InvalidInput, "beyond beginning of file")
                } else if self.offset as i64 + offset > self.size() as i64 {
                    ioerr!(InvalidInput, "beyond end of file")
                } else {
                    self.offset = (self.offset as i64 + offset) as usize;
                    Ok(self.offset as u64)
                }
            }
        }
    }
}

impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use crate::traits::{FileSystem, Dir as DirTrait, Entry as EntryTrait};

        let mut bytes_written = 0usize;
        let bytes_to_write = buf.len();
        let mut cluster = match self.current_cluster{
            Some(cluster)=> cluster,
            None => return ioerr!(InvalidInput, "cluster in chain marked reserved"),
        };
        let mut current_cluster_result = Ok(self.current_cluster);
        let mut current_cluster = current_cluster_result?;
        let mut cluster_offset = self.offset % self.bytes_per_cluster;
        while self.vfat.lock(
            |vfat| { vfat.size_to_chain_end(cluster.fat_address()) }
        )? < self.offset + buf.len() {
            let new_cluster = self.vfat.lock(
                |vfat| {
                    vfat.find_free_cluster()
                }
            )?;
            self.vfat.lock(
                |vfat| {
                    vfat.add_cluster_to_chain(cluster, new_cluster)
                }
            )?;
        }
        while bytes_written < bytes_to_write {
            let bytes = self.vfat.lock(
                |vfat| {
                    vfat.write_cluster(
                        current_cluster.unwrap(),
                        cluster_offset,
                        &buf[bytes_written..]
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
            bytes_written += bytes;
            cluster_offset = 0;
        }
        self.current_cluster = current_cluster;
        self.offset += bytes_written;
        if self.offset > self.size {
            self.size = self.offset;
            let rootdir_cluster = self.vfat.lock(|vfat| vfat.rootdir_cluster);
            let mut entry = Entry::Dir(Dir {
                vfat: self.vfat.clone(),
                first_cluster: rootdir_cluster,
                name: String::from(""),
                metadata: Metadata::default(),
                size: 0,
                path: PathBuf::from("/"),
                parent_path: None,
                parent_first_cluster: None,
            });
            for component in self.parent_path.as_path().components() {
                match component {
                    // Component::ParentDir => {
                    //     entry = entry.into_dir()
                    //         .ok_or(newioerr!(InvalidInput, "path parent is not dir"))?
                    //         .find("..")?;
                    // },
                    Component::Normal(name) => {
                        entry = entry.into_dir()
                            .ok_or(newioerr!(NotFound, "path not found"))?
                            .find(name)?;
                    }
                    _ => (),
                }
            }

            let dir_entry = match entry.into_dir() {
                Some(dir_entry) => dir_entry,
                None => return ioerr!(InvalidData, "parent_dir is not an Entry::Dir"),
            };
            let mut dir_iter = dir_entry.entries()?;
            let mut fat_buf: Vec<u8> = Vec::new();
            let size = dir_iter.write_entry_size(self.name.as_str(), self.size, &mut fat_buf)?;
            let chain_size = self.vfat.lock(|vfat| {
                vfat.write_chain(self.parent_first_cluster, &fat_buf)
            })?;
            // TODO convert parent_dir_entry into Dir and then to DirIterator
            // TODO call set_entry_size with new size on DirIterator
            // TODO call write_dir_entries on DirIterator
        }
        Ok(bytes_written)
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
