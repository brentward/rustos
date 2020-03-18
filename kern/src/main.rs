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

use console::kprintln;
use pi::timer;
use core::time::Duration;

use allocator::Allocator;
use fs::FileSystem;
use process::GlobalScheduler;
use traps::irq::Irq;
use vm::VMManager;
use aarch64;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
pub static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
pub static VMM: VMManager = VMManager::uninitialized();
pub static IRQ: Irq = Irq::uninitialized();

#[no_mangle]
fn kmain() -> ! {

    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }
    // let current_el = unsafe { aarch64::current_el() };
    pi::timer::spin_sleep(Duration::from_millis(250));
    // kprintln!("Current Exception Level: {}", current_el);
    kprintln!("test is test");
    // pi::timer::spin_sleep(Duration::from_secs(1));
    aarch64::brk!(2);
    // unsafe { asm!("brk 2" :::: "volatile"); }
    kprintln!("Welcome to BrentOS");

    kprintln!("Welcome to BrentOS");
    shell::shell("> ");
}
