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
use kernel_api::syscall::{open, exit};

use core::time::Duration;
use alloc::string::String;
use alloc::vec;


fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    match open("/config.txt") {
        Ok(fid) => println!("fid: {}", fid),
        Err(e) => println!("{:#?}", e),
    }

    let foo = vec![1, 2, 3];
    println!("foo is {:?}", foo);
    let mut bar = String::from("This is a string and it is on the heap");
    println!("bar is {}", bar);
    bar.push_str(". And I added this to the string!");
    println!("now bar is: {}", bar);

}
