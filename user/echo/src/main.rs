#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(panic_info_message)]
#![feature(never_type)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::allocator::Allocator;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();

use core::time::Duration;

use kernel_api::syscall::*;
use kernel_api::{print, println, OsResult, SocketStatus, OsError};
use bw_allocator::Allocator;

#[global_allocator]
pub static A: Allocator = Allocator::new();

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

fn main() {
    let result = main_inner();
    if let Err(error) = result {
        let mut s = String::new();
        write!(s, "Terminating with error: {:?}\r\n", error);
        print!("{}", s);
    }
}

fn main_inner() -> OsResult<!> {
    let socket = sock_create();
    sock_listen(socket, 80)?;
    let mut status = sock_status(socket)?;
    while !status.can_send {
        let mut s = String::new();
        write!(s, "Waiting for {:?}: {:?} to be able to send.\r\n", socket, status);
        print!("{}", s);
        let _sleep_duration = sleep(Duration::from_secs(1))?;
        status = sock_status(socket)?;
    }
    let message = "Welcome to Echo server hosted on RustOS!\r\n";
    let _bytes_sent = sock_send(socket, message.as_bytes())?;
    loop {
        let mut buf = [0u8; 1024];
        let _bytes_recvd = sock_recv(socket, &mut buf)?;
        let in_message = core::str::from_utf8(&buf).map_err(|_| OsError::IoErrorInvalidData)?;
        print!("{}", in_message);
        let _bytes_sent = sock_send(socket, &buf)?;
    }
}
