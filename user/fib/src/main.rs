#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{getpid, time, sleep, exit};

use core::time::Duration;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    println!("Started...");

    let pid = getpid();
    println!("PID: {}", pid);

    let current_time = time();
    println!("time in milliseconds: {}", current_time.as_millis());


    let rtn = fib(40);

    println!("Ended: Result = {}", rtn);

    let current_time = time();
    println!("time in milliseconds: {}", current_time.as_millis());

    println!("sleep for 5 sec");
    sleep(Duration::from_secs(5));

    println!("I'm back");
    println!("another fib");
    let rtn = fib(40);

    println!("Ended: Result = {}", rtn);

    println!("goodbye");

    exit()
}
