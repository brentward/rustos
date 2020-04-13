use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};
use core::fmt;
use shim::io;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    File(File<HANDLE>),
    Dir(Dir<HANDLE>),
}

impl<HANDLE: VFatHandle> Entry<HANDLE> {
    pub fn size(&self) -> usize {
        match self {
            Entry::File(file) => file.size,
            Entry::Dir(dir) => dir.size,
        }
    }

    pub fn display_name(&self) -> String {
        use traits::Entry;

        if self.is_dir() {
            let mut name = String::from(self.name());
            name.push('/');
            name
        } else {
            String::from(self.name())
        }
    }

    pub fn write_size(&self, to: &mut String) -> fmt::Result {
        use fmt::Write;

        write!(to, "{}", self.size())
    }

    pub fn write_human_size(&self, to: &mut String) -> fmt::Result {
        use fmt::Write;

        match self.size() {
            size@ 0..=1023 => {
                write!(to, "{} B", size.to_string())
            }
            size@ 1024..=1_048_575 => {
                write!(to, "{} KiB", (size / 1024).to_string())
            }
            size@ 1_048_576..=1_073_741_823 => {
                write!(to, "{} MiB", (size / 1_048_576).to_string())
            }
            size => {
                write!(to, "{} GiB", (size / 1_073_741_824).to_string())
            }
        }
    }

    // /// They byte array representing the metadata as a string of raw
    // /// VFatDirEntries
    // pub fn write_metadata(&self, buf: &mut [u8]) -> io::Result<usize> {
    //     Ok(0)
    // }
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;

    fn name(&self) -> &str {
        match self {
            Entry::File(file) => file.name.as_str(),
            Entry::Dir(dir) => dir.name.as_str(),
        }
    }

    fn metadata(&self) -> &Self::Metadata {
        match self {
            Entry::File(file) => &file.metadata,
            Entry::Dir(dir) => &dir.metadata,
        }
    }

    fn as_file(&self) -> Option<&<Self as traits::Entry>::File> {
        match &self {
            Entry::File(file) => Some(file),
            _ => None,
        }
    }

    fn as_dir(&self) -> Option<&<Self as traits::Entry>::Dir> {
        match &self {
            Entry::Dir(dir) => Some(dir),
            _ => None,
        }
    }

    fn into_file(self) -> Option<<Self as traits::Entry>::File> {
        match self {
            Entry::File(file) => Some(file),
            _ => None,
        }
    }

    fn into_dir(self) -> Option<<Self as traits::Entry>::Dir> {
        match self {
            Entry::Dir(dir) => Some(dir),
            _ => None,
        }
    }
}

impl<HANDLE: VFatHandle> fmt::Display for Entry<HANDLE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use traits::Entry;
        use fmt::Write;

        write!(
            f,
            "{}  {:<8}  {} \r\n",
            self.metadata().to_string(),
            self.size(),
            self.display_name(),
        )
    }
}
