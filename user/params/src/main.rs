#![feature(asm)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::{println, print};
use bw_allocator::Allocator;
use alloc::vec::Vec;
use alloc::string::String;
use kernel_api::syscall::args;

#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    // println!("addr: {}", addr);
    // let len = unsafe { kernel_api::args::args_len(addr as *const u8) };
    // for idx in 0..len {
    //     let byte = unsafe { *((addr + idx as u64) as *const u8) };
    //     print!("{}, ", byte);
    //     if byte == 0 {
    //         println!("");
    //     }
    // }
    println!("calling args()");
    let args = args();
    for arg in args {
        println!("{}", arg);
    }
    // for arg in arg_v {
    //     println!("{}", arg);
    // }
    //
    //
    println!("Hello, world!");
    // println!("{}", unsafe { cr0::ARGS_ADDR });
    // let args = cr0::args();
    println!("doing stuff");
    // let args_v = args.into_iter().map(|arg|String::from(arg)).collect::<Vec<_>>();
    //
    // for arg in args_v {
    //     println!("argv: {}", arg);
    // }
    let mut new_vec = Vec::new();
    for idx in 0..10 {
        use core::fmt::Write;
        let mut item = String::new();
        write!(item, "new aasdfsdfasdfsdaasdfasdfrg: {}", idx);
        new_vec.push(item);
    }
    for item in new_vec {
        println!("item: {}", item);
    }
    // let args = cr0::args();
    // for arg in args {
    //     println!("{}", arg);
    //     let len = arg.len();
    //     let cap = arg.capacity();
    //     println!("ptr: {:?}, len: {}, cap: {}", &arg as *const String, len, cap);
    //
    // }
}
