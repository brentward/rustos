#![feature(asm)]
#![feature(panic_info_message)]
#![feature(never_type)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::{print, println, OsError, OsResult};
use kernel_api::fs::Handle;
use kernel_api::syscall::*;
use bw_allocator::Allocator;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::cr0::{STD_IN, STD_OUT};

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const BELL: u8 = 7;
const BACK: u8 = 8;
const DEL: u8 = 127;


#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    let result = main_inner();
    if let Err(error) = result {
        let mut s = String::new();
        write!(s, "Terminating with error: {:?}\r\n", error);
        print!("{}", s);
    }
}

fn main_inner() -> OsResult<!> {
    loop {
        let out_str = String::from("> ");
        write(&STD_OUT, out_str.as_bytes())?;
        const BUF_LEN: usize = 512;
        let mut cmd_buf = Vec::with_capacity(BUF_LEN);
        let mut bytes = 0;
        'read_char: loop {
            let mut buf = [0u8];
            let b = read(&STD_IN, &mut buf)?;
            match buf[0] {
                DEL | BACK => {
                    if cmd_buf.len() > 0 {
                        cmd_buf.pop();
                        write(&STD_OUT, &[BACK, b' ', BACK])?;
                    } else {
                        write(&STD_OUT, &[BELL])?;
                    }
                }
                CR | LF => break 'read_char,
                byte if byte < 32 || byte > 127 => {
                    write(&STD_OUT, &[BELL])?;
                },
                byte => {
                    if cmd_buf.len() < BUF_LEN {
                        cmd_buf.push(byte);
                        write(&STD_OUT, &[byte])?;
                    } else {
                        write(&STD_OUT, &[BELL])?;
                    }
                }

            }
        }
        let input_str = core::str::from_utf8(cmd_buf.as_slice())
            .expect("input bytes failed to cast back to string");
        write(&STD_OUT, &[LF])?;
        write(&STD_OUT, input_str.as_bytes())?;
        write(&STD_OUT, &[LF])?;
    }
}
