use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};
use core::fmt;
use alloc::string::{String, ToString};

#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    File(File<HANDLE>),
    Dir(Dir<HANDLE>),
}

pub struct HumanReadableEntry<HANDLE: VFatHandle> {
    pub entry: Entry<HANDLE>
}

impl<HANDLE: VFatHandle> Entry<HANDLE> {
    pub fn size(&self) -> usize {
        match self {
            Entry::File(file) => file.size,
            Entry::Dir(dir) => dir.size,
        }
    }
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

        let name = if self.is_dir() {
            let mut name = String::from(self.name());
            name.push('/');
            name
        } else {
            String::from(self.name())
        };
        let metadata = self.metadata().to_string();
        let size = get_size(self.size(), false);
        write!(f, "{}  {:<8}  {} \r\n", metadata, size, name)
    }
}

impl<HANDLE: VFatHandle> fmt::Display for HumanReadableEntry<HANDLE> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use traits::Entry;
        use fmt::Write;

        let name = if self.entry.is_dir() {
            let mut name = String::from(self.entry.name());
            name.push('/');
            name
        } else {
            String::from(self.entry.name())
        };
        let metadata = self.entry.metadata().to_string();
        let size = get_size(self.entry.size(), true);
        write!(f, "{}  {:<8}  {} \r\n", metadata, size, name)
    }
}

fn get_size(size: usize, human_redable: bool) -> String {
    use fmt::Write;

    let mut result = String::new();
    if human_redable {
        match size {
            size@ 0..=1023 => {
                write!(result, "{} B", size.to_string());
            }
            size@ 1024..=1_048_575 => {
                write!(result, "{} KiB", (size / 1024).to_string());
            }
            size@ 1_048_576..=1_073_741_823 => {
                write!(result, "{} MiB", (size / 1_048_576).to_string());
            }
            size => {
                write!(result, "{} GiB", (size / 1_073_741_824).to_string());
            }
        }
    } else {
        write!(result, "{}", size);
    }
    result
}
