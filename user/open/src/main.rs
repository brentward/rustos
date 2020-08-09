#![feature(asm)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]
extern crate alloc;

mod cr0;

use kernel_api::syscall::{getpid, time, open, read, getdents, sleep, stat};
use kernel_api::{fs, print, println};
use bw_allocator::Allocator;

use core::time::Duration;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use shim::io::Write;
use shim::path::PathBuf;
use kernel_api::fs::{Metadata, Stat};
// use core::fmt::Write as FmtWrite;

#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    print!("running open\r\n");
    print!("reading root:\r\n");

    let mut dent_buf = Vec::new();
    let mut offset = 0u64;
    let path = PathBuf::from("/");

    loop {
        dent_buf.clear();
        for _ in 0..16 {
            dent_buf.push(fs::DirEnt::default());
        }

        match getdents("/", &mut dent_buf, offset) {
            Ok(entries) => {
                for index in 0u64..entries {
                    let mut path = path.clone();
                    let name = dent_buf[index as usize].name().unwrap();

                    path.push(name);
                    let mut stat_buf = [Stat::default()];
                    match stat(path, &mut stat_buf) {
                        Ok(_) => (),
                        Err(e) =>  println!("Error stat: {:#?}", e),
                    }
                    let stat = stat_buf[0];



                    match dent_buf[index as usize].d_type() {
                        fs::DirType::Dir => println!("{} {} {}/", stat.metadata(), stat.size(), name),
                        fs::DirType::File => println!("{} {} {}",  stat.metadata(), stat.size(), name),
                        fs::DirType::None => println!("huh?"),
                    }
                }
                offset += entries;
                if entries == 0 {
                    break
                }
            },
            Err(e) => println!("Error getdent: {:#?}", e),
        }
    }
    print!("/nf.txt:\r\n");
    sleep(Duration::from_secs(5));

    match open("/nf.txt") {
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

    // let mut buf = [0u8];
    // let bytes = match read(0, &mut buf) {
    //     Ok(bytes) => println!("bytes in buf: {}", buf[0]),
    //     Err(_) => println!("Error getting from file 0"),
    // };
}
