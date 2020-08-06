#![feature(asm)]
#![no_std]
#![no_main]
extern crate alloc;

mod cr0;

use kernel_api::print;
use kernel_api::syscall::getpid;
use bw_allocator::Allocator;

#[global_allocator]
pub static A: Allocator = Allocator::new();

use alloc::string::String;
use core::fmt::Write;

fn main() {
    let pid = getpid();
    let mut string_out = String::new();
    write!(string_out, "[{:02}] Hello, world!\r\n", pid).expect("write macro error");
    print!("{}", string_out);
}
