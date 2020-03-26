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
    fn print_init_with_progress(msg: &str) {
        kprint!("{}", msg);
        let fake_random = (pi::timer::current_time().as_micros() & 0xFF) as u64 + 0xFF;
        let align_to = 40 - msg.len() as u64;
        for index in 0..align_to {
            pi::timer::spin_sleep(core::time::Duration::from_millis(fake_random / align_to));
            kprint!(".")
        }
    }
    pi::timer::spin_sleep(core::time::Duration::from_millis(250));
    unsafe {
        print_init_with_progress("Initializing ALLOCATOR");
        ALLOCATOR.initialize();
        kprintln!(" [ok]");
        print_init_with_progress("Initializing FILESYSTEM");
        FILESYSTEM.initialize();
        kprintln!(" [ok]");
        print_init_with_progress("Initializing IRQ");
        IRQ.initialize();
        kprintln!(" [ok]");
        print_init_with_progress("Initializing SCHEDULER");
        SCHEDULER.initialize();
        kprintln!(" [ok]");
        print_init_with_progress("Starting SCHEDULER");
        kprintln!(" [ok]");
        kprintln!("");
        kprintln!("Welcome to BrentOS");
        SCHEDULER.start()
    }
}
