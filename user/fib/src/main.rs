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
use kernel_api::syscall::{getpid, time};

use core::time::Duration;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    println!("Started...");

    let rtn = fib(40);

    println!("Ended: Result = {}", rtn);
}
