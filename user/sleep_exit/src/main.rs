#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{sleep, exit};

use core::time::Duration;

fn main() {
    sleep(Duration::from_secs(10));
    exit()
}
