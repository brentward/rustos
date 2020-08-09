use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::write_volatile;

use kernel_api::println;
use kernel_api::fs::Handle;

pub static STD_IN: Handle = Handle::StdIn;
pub static STD_OUT: Handle = Handle::StdOut;
pub static STD_ERR: Handle = Handle::StdErr;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic");
    match info.location() {
        Some(location) => {
            println!("FILE: {}", location.file());
            println!("LINE: {}", location.line());
            println!("COL: {}", location.column());
        }
        None => println!("Panic location cannot be determined"),
    }
    println!("");
    match info.message() {
        Some(message) => println!("{}", message),
        None => println!("Panic message cannot be determined"),
    }
    loop {}
}

unsafe fn zeros_bss() {
    extern "C" {
        static mut __bss_beg: u64;
        static mut __bss_end: u64;
    }

    let mut iter: *mut u64 = &mut __bss_beg;
    let end: *mut u64 = &mut __bss_end;

    while iter < end {
        write_volatile(iter, zeroed());
        iter = iter.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    zeros_bss();
    crate::main();
    kernel_api::syscall::exit();
}
