#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(ptr_internals)]
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
pub mod param;
pub mod process;
pub mod traps;
pub mod vm;

use console::{kprintln, kprint};

use allocator::Allocator;
use fs::FileSystem;
use process::GlobalScheduler;
use traps::irq::Irq;
use vm::VMManager;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
pub static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
pub static VMM: VMManager = VMManager::uninitialized();
pub static IRQ: Irq = Irq::uninitialized();

#[no_mangle]
fn kmain() -> ! {
    pi::timer::spin_sleep(core::time::Duration::from_millis(250));
    unsafe {
        kprint!("Initializing ALLOCATOR... ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(250));
        ALLOCATOR.initialize();
        kprintln!("[ok]");
        kprint!("Initializing FILESYSTEM... ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(250));
        FILESYSTEM.initialize();
        kprintln!("[ok]");
        kprint!("Initializing IRQ... ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(250));
        IRQ.initialize();
        kprintln!("[ok]");
        kprint!("Initializing SCHEDULE... ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(250));
        SCHEDULER.initialize();
        kprintln!("[ok]");
        kprint!("Starting SCHEDULER... ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(250));
        SCHEDULER.start()
    }
}
