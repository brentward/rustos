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
use kernel_api::syscall::{open, exit, read};

use core::time::Duration;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use shim::io::Write;


fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    let open_result =  open("/nerdfonts.txt");
    match open_result {
        Ok(fid) => {
            let mut file_vec = Vec::new();
            let mut bytes = 0usize;
            let mut bytes_read = 0usize;
            loop {
                let mut file_buf = [0u8; 256];
                bytes = match read(fid, &mut file_buf){
                    Ok(bytes) => bytes,
                    Err(e) => {
                        println!("{:?}", e);
                        0
                    }
                };
                if bytes == 0 {
                    break
                }
                bytes_read += bytes;
                let _bytes_written = file_vec.write(&file_buf)
                    .expect("failed to write to vector");



            }
            while file_vec.len() > bytes_read {
                file_vec.pop();
            }
            match String::from_utf8(file_vec) {
                Ok(string) => {
                    println!("{}", string);
                },
                Err(_) => println!("Error converting file_vec to string"),
            };


        },
        Err(e) => println!("{:#?}", e),
    }
}
