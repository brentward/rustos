#![feature(asm)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]
extern crate alloc;

mod cr0;

use kernel_api::syscall::{getpid, time, open, read, sleep, stat, args};
use kernel_api::{fs, print, println};
use bw_allocator::Allocator;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use shim::io::Write;

#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    let args = args();
    match args.get(1) {
        Some(path) => {
            match open(path) {
                Ok(handle) => {
                    let mut bytes = 0usize;
                    let mut bytes_read = 0usize;
                    let mut file_vec= Vec::new();
                    loop {
                        let mut file_buf = [0u8; 256];
                        bytes = match read(&handle, &mut file_buf){
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
        None => println!("cat: open <path>"),
    }
}
