use core::mem::zeroed;
use core::panic::PanicInfo;
use core::alloc::Layout;
use core::ptr::write_volatile;
use crate::ALLOCATOR;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
pub fn oom(_layout: Layout) -> ! {
    panic!("OOM");
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
    unsafe { ALLOCATOR.initialize() };
    crate::main();
    kernel_api::syscall::exit();
}
