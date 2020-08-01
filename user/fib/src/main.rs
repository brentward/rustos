#![feature(asm)]
#![no_std]
#![no_main]
extern crate alloc;

mod cr0;

use kernel_api::{println, print};
use kernel_api::syscall::{getpid, time, rand};
use bw_allocator::Allocator;

#[global_allocator]
pub static A: Allocator = Allocator::new();

use core::time::Duration;
use alloc::string::String;
use core::fmt::Write;

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
    let mut string_out = String::new();
    write!(string_out, "[{:02}] Started: {:?}\n\r", pid, beg).expect("write macro error");
    // println!("[{:02}] Started: {:?}", pid, beg);
    print!("{}", string_out);
    let die_throw = rand(1, 7);
    string_out.clear();
    write!(string_out, "[{:02}] Die: {}\n\r", pid, die_throw).expect("write macro error");
    print!("{}", string_out);

    let rtn = fib(40);

    let end = time();
    string_out.clear();
    write!(string_out, "[{:02}] Ended: {:?}\n\r", pid, end).expect("write macro error");
    write!(string_out, "[{:02}] Result: {} ({:?})\n\r", pid, rtn, end - beg);
    print!("{}", string_out);

    // println!("[{:02}] Ended: {:?}", pid, end);
    // println!("[{:02}] Result: {} ({:?})", pid, rtn, end - beg);
}
