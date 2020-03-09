#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(raw_vec_internals)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(panic_info_message)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

use console::kprintln;
use pi::{timer, gpio, uart};
use core::time::Duration;
use alloc::vec::Vec;
// use core::fmt::Write;

use allocator::Allocator;
use fs::FileSystem;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();

#[no_mangle]
fn kmain() -> ! {

    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }
    use fs::traits::{FileSystem, Dir, Entry};
    let root_dir = FILESYSTEM.open_dir("/").unwrap();
    pi::timer::spin_sleep(Duration::from_secs(2));
    let entries = root_dir.entries().unwrap().collect::<Vec<_>>();
    for entry in entries {
        if entry.is_file() {
            kprintln!("{:?}", entry.into_file());
        } else {
            kprintln!("{:?}", entry.into_dir());
        }
    }
    shell::shell("> ");
}
