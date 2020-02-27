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
use pi::{timer, gpio, uart};
use core::time::Duration;


// FIXME: You need to add dependencies here to
// test your drivers (Phase 2). Add them as needed.
// const GPIO_BASE: usize = 0x3F000000 + 0x200000;
//
// const GPIO_FSEL1: *mut u32 = (GPIO_BASE + 0x04) as *mut u32;
// const GPIO_SET0: *mut u32 = (GPIO_BASE + 0x1C) as *mut u32;
// const GPIO_CLR0: *mut u32 = (GPIO_BASE + 0x28) as *mut u32;

#[no_mangle]
fn kmain() -> ! {
    loop {
        let mut pi_uart = uart::MiniUart::new();
        let byte_read = pi_uart.read_byte();
        pi_uart.write_byte(byte_read);
    }
}
