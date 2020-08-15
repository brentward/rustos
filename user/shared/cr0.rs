use core::mem::zeroed;
use core::panic::PanicInfo;
use core::ptr::write_volatile;
// use alloc::vec::Vec;
// use alloc::string::String;
// use alloc::boxed::Box;
// use core::str::from_utf8;
// use core::slice;


use kernel_api::println;
use kernel_api::fs::Handle;
// use kernel_api::args::CArgs;
// use kernel_api::syscall;

pub static STD_IN: Handle = Handle::StdIn;
pub static STD_OUT: Handle = Handle::StdOut;
pub static STD_ERR: Handle = Handle::StdErr;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("");
    println!(r#"     ,--.!,"#);
    println!(r#"  __/   -*-"#);
    println!(r#",d08b.  '|`"#);
    println!(r#"0088MM"#);
    println!(r#"`9MMP'"#);
    println!("");
    println!("PANIC!");
    match info.location() {
        Some(location) => {
            println!("  File: {}", location.file());
            println!("  Line: {}", location.line());
            println!("  Column: {}", location.column());
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

    //
    // let mut args_addr: u64;
    // unsafe {
    //     asm!("mov $0, x0"
    //      : "=r"(args_addr)
    //      :: "x0"
    //      : "volatile");
    // }
    //
    // let args_len = kernel_api::args::args_len(args_addr as *mut u8);
    // let mut buf = Vec::<u8>::with_capacity(args_len);
    // use shim::io::Write;
    //
    // let slice = slice::from_raw_parts(args_addr as *mut u8, args_len);
    // let bytes = buf.write(slice).unwrap();
    // let args = unsafe { CArgs::from_vec_with_nul_unchecked(buf) };

    // let args = unsafe { CArgs::from_raw(args_addr as *mut u8) };
    // let mut args_v = Vec::new();
    // // args_v.push(String::from("argument_list"));
    // for arg in &args {
    //     args_v.push(arg);
    // }

    // let mut ptr = args_addr;
    // let mut args_v = Vec::new();
    // loop {
    //     if *(ptr as *const u8) == 0 {
    //         break
    //     }
    //     let len = kernel_api::cstr::str_len(ptr as *mut u8);
    //     let slice = slice::from_raw_parts(ptr as *mut u8, len);
    //     let arg = from_utf8(slice).unwrap();
    //     args_v.push(String::from(arg));
    //     ptr += len as u64 + 1;
    //
    // }
    crate::main();
    kernel_api::syscall::exit();
}
