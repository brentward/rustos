pub mod sd;

use alloc::rc::Rc;
use core::fmt::{self, Debug};
use shim::io;
use shim::ioerr;
use shim::path::Path;

pub use fat32::traits;
use fat32::vfat::{Dir, Entry, File, VFat, VFatHandle};

use self::sd::Sd;
use crate::mutex::Mutex;

#[derive(Clone)]
pub struct PiVFatHandle(Rc<Mutex<VFat<Self>>>);

// These impls are *unsound*. We should use `Arc` instead of `Rc` to implement
// `Sync` and `Send` trait for `PiVFatHandle`. However, `Arc` uses atomic memory
// access, which requires MMU to be initialized on ARM architecture. Since we
// have enabled only one core of the board, these unsound impls will not cause
// any immediate harm for now. We will fix this in the future.
unsafe impl Send for PiVFatHandle {}
unsafe impl Sync for PiVFatHandle {}

impl Debug for PiVFatHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "PiVFatHandle")
    }
}

impl VFatHandle for PiVFatHandle {
    fn new(val: VFat<PiVFatHandle>) -> Self {
        PiVFatHandle(Rc::new(Mutex::new(val)))
    }

    fn lock<R>(&self, f: impl FnOnce(&mut VFat<PiVFatHandle>) -> R) -> R {
        f(&mut self.0.lock())
    }
}
pub struct FileSystem(Mutex<Option<PiVFatHandle>>);

impl FileSystem {
    /// Returns an uninitialized `FileSystem`.
    ///
    /// The file system must be initialized by calling `initialize()` before the
    /// first memory allocation. Failure to do will result in panics.
    pub const fn uninitialized() -> Self {
        FileSystem(Mutex::new(None))
    }

    /// Initializes the file system.
    /// The caller should assure that the method is invoked only once during the
    /// kernel initialization.
    ///
    /// # Panics
    ///
    /// Panics if the underlying disk or file sytem failed to initialize.
    pub unsafe fn initialize(&self) {
        let block_device = Sd::new().expect("SD card failed to init");
        let vfat = VFat::<PiVFatHandle>::from(block_device).expect("VFat::from() on SD card block device failed");
        *self.0.lock() = Some(vfat);
    }
}

impl<'a> fat32::traits::FileSystem for &'a FileSystem {
    type File = File<PiVFatHandle>;
    type Dir = Dir<PiVFatHandle>;
    type Entry = Entry<PiVFatHandle>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        let handle = match *self.0.lock() {
            Some(ref handle) => handle.clone(),
            None => return ioerr!(NotConnected, "file system uninitialized"),
        };
        handle.open(path)
    }

    fn open_file<P: AsRef<Path>>(self, path: P) -> io::Result<Self::File> {
        let handle = match *self.0.lock() {
            Some(ref handle) => handle.clone(),
            None => return ioerr!(NotConnected, "file system uninitialized"),
        };
        handle.open_file(path)
    }

    fn open_dir<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Dir> {
        let handle = match *self.0.lock() {
            Some(ref handle) => handle.clone(),
            None => return ioerr!(NotConnected, "file system uninitialized"),
        };
        handle.open_dir(path)
    }
}
