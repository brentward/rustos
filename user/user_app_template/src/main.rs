#![feature(asm)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::println;
use bw_allocator::Allocator;

#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    println!("Hello, world!");
}
