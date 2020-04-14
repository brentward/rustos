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
use kernel_api::syscall::{open, exit, read, getdent};
use kernel_api::fs;

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
    let open_result =  open("/nf.txt");
    match open_result {
        Ok(fid) => println!("fid: {}", fid),
        Err(e) => println!("e: {:#?}", e),
    }
    // match open_result {
    //     Ok(fid) => {
    //         let mut file_vec = Vec::new();
    //         let mut bytes = 0usize;
    //         let mut bytes_read = 0usize;
    //         loop {
    //             let mut file_buf = [0u8; 256];
    //             bytes = match read(fid, &mut file_buf){
    //                 Ok(bytes) => bytes,
    //                 Err(e) => {
    //                     println!("{:?}", e);
    //                     0
    //                 }
    //             };
    //             if bytes == 0 {
    //                 break
    //             }
    //             bytes_read += bytes;
    //             let _bytes_written = file_vec.write(&file_buf)
    //                 .expect("failed to write to vector");
    //
    //
    //
    //         }
    //         while file_vec.len() > bytes_read {
    //             file_vec.pop();
    //         }
    //         match String::from_utf8(file_vec) {
    //             Ok(string) => {
    //                 println!("{}", string);
    //             },
    //             Err(_) => println!("Error converting file_vec to string"),
    //         };
    //
    //
    //     },
    //     Err(e) => println!("{:#?}", e),
    // }
    //
    // match open("/") {
    //     Ok(fd) => {
    //         // let mut dent_buf = [fs::DirEnt::default(); 32];
    //         let mut dent_buf = Vec::new();
    //         for _ in 0..64 {
    //             dent_buf.push(fs::DirEnt::default());
    //         }
    //         match getdent(fd, &mut dent_buf) {
    //             Ok(entries) => {
    //                 for index in 0..entries {
    //                     let name = dent_buf[index].name().unwrap();
    //                     match dent_buf[index].d_type() {
    //                         fs::DirType::Dir => println!("{}/", name),
    //                         fs::DirType::File => println!("{}", name),
    //                         fs::DirType::None => println!("huh?"),
    //                     }
    //                 }
    //             },
    //             Err(e) => println!("Error getdent: {:#?}", e),
    //         }
    //     }
    //     Err(e) => println!("{:#?}", e),
    // }
    // let mut buf = [0u8];
    // let bytes = match read(0, &mut buf) {
    //     Ok(bytes) => println!("bytes in buf: {}", buf[0]),
    //     Err(_) => println!("Error getting from file 0"),
    // };

}
