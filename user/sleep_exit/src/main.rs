#![feature(alloc_error_handler)]
#![feature(asm)]
#![no_std]
#![no_main]
extern crate alloc;

use kernel_api::allocator::Allocator;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{sleep, exit};

use core::time::Duration;

fn main() {
    sleep(Duration::from_secs(10));
    exit()
}
