#![feature(asm)]
#![no_std]
#![no_main]
extern crate alloc;

mod cr0;

use kernel_api::{println, print};
use kernel_api::syscall::{getpid, time};
use bw_allocator::Allocator;

#[global_allocator]
pub static A: Allocator = Allocator::new();

use core::time::Duration;
use alloc::string::String;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    let pid = getpid();
    let beg = time();
    println!("[{:02}] Started: {:?}", pid, beg);
    let heap_string = String::from("this is from the heap");
    println!("{}", heap_string);
    let rtn = fib(40);

    let end = time();
    println!("[{:02}] Ended: {:?}", pid, end);
    println!("[{:02}] Result: {} ({:?})", pid, rtn, end - beg);
}
