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

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE>
    where HANDLE: std::marker::Copy
{
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;

    fn name(&self) -> &str {
        panic!("Entry::name()")
    }
    fn metadata(&self) -> &Self::Metadata {
        panic!("Entry::metadata()")
    }
    fn as_file(&self) -> Option<&<Self as traits::Entry>::File> {
        panic!("Entry::as_file()")
    }
    fn as_dir(&self) -> Option<&<Self as traits::Entry>::Dir> {
        panic!("Entry::as_dir()")
    }
    fn into_file(self) -> Option<<Self as traits::Entry>::File> {
        panic!("Entry::into_file()")
    }
    fn into_dir(self) -> Option<<Self as traits::Entry>::Dir> {
        panic!("Entry::into_dir()")
    }
}
