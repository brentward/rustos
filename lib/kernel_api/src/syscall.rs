use core::fmt;
use core::fmt::Write;
use core::time::Duration;

use crate::*;

macro_rules! err_or {
    ($ecode:expr, $rtn:expr) => {{
        let e = OsError::from($ecode);
        if let OsError::Ok = e {
            Ok($rtn)
        } else {
            Err(e)
        }
    }};
}

pub fn sleep(span: Duration) -> OsResult<Duration> {
    if span.as_millis() > core::u64::MAX as u128 {
        panic!("too big!");
    }

    let ms = span.as_millis() as u64;
    let mut ecode: u64;
    let mut elapsed_ms: u64;

    unsafe {
        asm!("mov x0, $2
              svc $3
              mov $0, x0
              mov $1, x7"
             : "=r"(elapsed_ms), "=r"(ecode)
             : "r"(ms), "i"(NR_SLEEP)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, Duration::from_millis(elapsed_ms))
}

pub fn time() -> Duration {
    let mut elapsed_s: u64 = 0;
    let mut fractional_ns: u64 = 0;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x1"
             : "=r"(elapsed_s), "=r"(fractional_ns)
             : "i"(NR_TIME)
             : "x0", "x1"
             : "volatile");
    }

    Duration::from_secs(elapsed_s) + Duration::from_nanos(fractional_ns)
}

pub fn exit() -> ! {
    unsafe {
        asm!("svc $0" :: "i"(NR_EXIT) :: "volatile");
    }
    loop { }
}

pub fn write(b: u8) {
    if !b.is_ascii() {
        panic!("{} is not valid ascii", b)
    }
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $0
              svc $1"
             :: "r"(b), "i"(NR_WRITE)
             :: "volatile");
    }
}

pub fn getpid() -> u64 {
    let mut ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!("svc $1
              mov $0, x0"
             : "=r"(pid)
             : "i"(NR_GETPID)
             : "x0"
             : "volatile");
    }

    pid
}

pub fn open(path: &str) -> OsResult<u64> {

    if !path.is_ascii() {
        panic!("Path: {} is not valid ascii", path)
    }
    let path_ptr = path.as_ptr() as u64;
    let path_len = path.len() as u64;
    let mut ecode: u64;
    let mut fid: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(fid), "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "i"(NR_OPEN)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, fid)

}

pub fn read(fd: u64, buf: &mut [u8]) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut bytes: usize;
    let count = buf.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(bytes), "=r"(ecode)
             : "r"(fd), "r"(buf_ptr), "r"(count), "i"(NR_READ)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn getdent(fd: u64, buf: &mut [fs::DirEnt]) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut entries: usize;
    let count = buf.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(entries), "=r"(ecode)
             : "r"(fd), "r"(buf_ptr), "r"(count), "i"(NR_GETDENT)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, entries)
}

pub fn sbrk(size: u64) -> OsResult<*mut u8> {
    let mut ecode: u64;
    let mut ptr: u64;

    unsafe {
        asm!("mov x0, $2
              svc $3
              mov $0, x0
              mov $1, x7"
             : "=r"(ptr), "=r"(ecode)
             : "r"(size), "i"(NR_SBRK)
             : "x0", "x7"
             : "volatile");
    }
    let ptr = ptr as *mut u8;
    err_or!(ecode, ptr)
}

struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            write(b);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::syscall::vprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
 () => (print!("\n"));
    ($($arg:tt)*) => ({
        $crate::syscall::vprint(format_args!($($arg)*));
        $crate::print!("\n");
    })
}

pub fn vprint(args: fmt::Arguments) {
    let mut c = Console;
    c.write_fmt(args).unwrap();
}
