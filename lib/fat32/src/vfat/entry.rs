use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};
use core::fmt;

// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    File(File<HANDLE>),
    Dir(Dir<HANDLE>),
}

// TODO: Implement any useful helper methods on `Entry`.

// #[derive(Debug)]
// pub struct Entry<HANDLE: VFatHandle> {
//     entry: EntryData<HANDLE>,
//     name: String,
//     metadata: Metadata,
//     size: u32,
// }

impl<HANDLE: VFatHandle> Entry<HANDLE> {
    pub fn size(&self) -> usize {
        match self {
            Entry::File(file) => file.size,
            Entry::Dir(dir) => dir.size,
        }
    }
}
//     pub fn from_file(
//         file: File<HANDLE>,
//         name: String,
//         metadata: Metadata,
//         size: u32
//     ) -> Entry<HANDLE> {
//         Entry {
//             entry: EntryData::File(file),
//             name,
//             metadata,
//             size
//         }
//     }
//
//     pub fn from_dir(
//         dir: Dir<HANDLE>,
//         name: String,
//         metadata: Metadata,
//         size: u32
//     ) -> Entry<HANDLE> {
//         Entry {
//             entry: EntryData::Dir(dir),
//             name,
//             metadata,
//             size
//         }
//     }
// }

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
