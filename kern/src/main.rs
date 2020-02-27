#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

pub mod console;
pub mod mutex;
pub mod shell;

use console::kprintln;
use pi::timer;
use core::time::Duration;


// FIXME: You need to add dependencies here to
// test your drivers (Phase 2). Add them as needed.
const GPIO_BASE: usize = 0x3F000000 + 0x200000;

const GPIO_FSEL1: *mut u32 = (GPIO_BASE + 0x04) as *mut u32;
const GPIO_SET0: *mut u32 = (GPIO_BASE + 0x1C) as *mut u32;
const GPIO_CLR0: *mut u32 = (GPIO_BASE + 0x28) as *mut u32;

#[no_mangle]
unsafe fn kmain() -> ! {
    GPIO_FSEL1.write_volatile((GPIO_FSEL1.read_volatile() & !(0b111 << 18)) | (0b001 << 18));
    loop {
        GPIO_SET0.write_volatile((GPIO_CLR0.read_volatile() & !(0b1 << 16)) | (0b1 << 16));
        timer::spin_sleep(Duration::from_millis(100));
        GPIO_CLR0.write_volatile((GPIO_CLR0.read_volatile() & !(0b1 << 16)) | (0b1 << 16));
        timer::spin_sleep(Duration::from_millis(900));
    }
}
