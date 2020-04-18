#![no_std]
#![feature(alloc_error_handler)]
#![feature(optin_builtin_traits)]

mod allocator;
use allocator::{bin, mutex::Mutex};

type AllocatorImpl = bin::Allocator;

use core::alloc::{GlobalAlloc, Layout};
use core::fmt;

/// `LocalAlloc` is an analogous trait to the standard library's `GlobalAlloc`,
/// but it takes `&mut self` in `alloc()` and `dealloc()`.
pub trait LocalAlloc {
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}

/// Thread-safe (locking) wrapper around a particular memory allocator.
pub struct Allocator(Mutex<Option<AllocatorImpl>>);

impl Allocator {
    /// Returns an `Allocator`.
    pub const fn new() -> Self {
        Allocator(Mutex::new(None))
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.0.lock().is_none() {
            *self.0.lock() = Some(AllocatorImpl::new());
        }
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if self.0.lock().is_none() {
            *self.0.lock() = Some(AllocatorImpl::new());
        }
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .dealloc(ptr, layout);
    }
}

impl fmt::Debug for Allocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.lock().as_mut() {
            Some(ref alloc) => write!(f, "{:?}", alloc)?,
            None => write!(f, "Not yet initialized")?,
        }
        Ok(())
    }
}

#[alloc_error_handler]
pub fn oom(_layout: Layout) -> ! {
    panic!("OOM");
}