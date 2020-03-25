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
    pi::timer::spin_sleep(core::time::Duration::from_millis(235));
    unsafe {
        kprint!("{:.<30} ", "Initializing ALLOCATOR");
        pi::timer::spin_sleep(core::time::Duration::from_millis(320));
        ALLOCATOR.initialize();
        kprintln!("[ok]");
        kprint!("{:.<30} ", "Initializing FILESYSTEM");
        pi::timer::spin_sleep(core::time::Duration::from_millis(128));
        FILESYSTEM.initialize();
        kprintln!("[ok]");
        kprint!("{:.<30} ","Initializing IRQ");
        pi::timer::spin_sleep(core::time::Duration::from_millis(356));
        IRQ.initialize();
        kprintln!("[ok]");
        kprint!("{:.<30} ", "Initializing SCHEDULER");
        pi::timer::spin_sleep(core::time::Duration::from_millis(389));
        SCHEDULER.initialize();
        kprintln!("[ok]");
        kprint!("{:.<30} ", "Starting SCHEDULER");
        pi::timer::spin_sleep(core::time::Duration::from_millis(400));
        kprintln!("[ok]");
        kprintln!("");
        kprintln!("Welcome to BrentOS");
        SCHEDULER.start()
    }
}
