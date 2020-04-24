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
    let mut _ecode: u64 = 0;
    let mut elapsed_s: u64 = 0;
    let mut fractional_ns: u64 = 0;

    unsafe {
        asm!("svc $3
              mov $0, x0
              mov $1, x1
              mov $2, x7"
             : "=r"(elapsed_s), "=r"(fractional_ns), "=r"(_ecode)
             : "i"(NR_TIME)
             : "x0", "x1", "x7"
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
    let mut _ecode: u64;

    unsafe {
        asm!("mov x0, $1
              svc $2
              mov $1, x7"
             : "=r"(_ecode)
             : "r"(b), "i"(NR_WRITE)
             : "x0", "x7"
             : "volatile");
    }
}

// pub fn write_str(msg: &str) -> OsResult<usize> {
pub fn write_str(msg: &str) {
    let msg_ptr = msg.as_ptr() as u64;
    let msg_len = msg.len() as u64;
    let mut _ecode: u64;
    let mut _len: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(_len), "=r"(_ecode)
             : "r"(msg_ptr), "r"(msg_len), "i"(NR_WRITE_STR)
             : "x0", "x1", "x7"
             : "volatile");
    }

    // err_or!(ecode, len as usize)
}

pub fn getpid() -> u64 {
    let mut _ecode: u64;
    let mut pid: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(pid), "=r"(_ecode)
             : "i"(NR_GETPID)
             : "x0", "x7"
             : "volatile");
    }

    pid
}

pub fn sbrk(size: usize) -> OsResult<*mut u8> {
    let mut ecode: u64;
    let mut ptr: u64;

    unsafe {
        asm!("mov x0, $2
              svc $3
              mov $0, x0
              mov $1, x7"
             : "=r"(ptr), "=r"(ecode)
             : "r"(size as u64), "i"(NR_SBRK)
             : "x0", "x7"
             : "volatile");
    }
    let ptr = ptr as *mut u8;
    err_or!(ecode, ptr)
}

pub fn sock_create() -> SocketDescriptor {
    // Lab 5 2.D
    unimplemented!("sock_create")
}

pub fn sock_status(descriptor: SocketDescriptor) -> OsResult<SocketStatus> {
    // Lab 5 2.D
    unimplemented!("sock_status")
}

pub fn sock_connect(descriptor: SocketDescriptor, addr: IpAddr) -> OsResult<()> {
    // Lab 5 2.D
    unimplemented!("sock_connect")
}

pub fn sock_listen(descriptor: SocketDescriptor, local_port: u16) -> OsResult<()> {
    // Lab 5 2.D
    unimplemented!("sock_listen")
}

pub fn sock_send(descriptor: SocketDescriptor, buf: &[u8]) -> OsResult<usize> {
    // Lab 5 2.D
    unimplemented!("sock_send")
}

pub fn sock_recv(descriptor: SocketDescriptor, buf: &mut [u8]) -> OsResult<usize> {
    // Lab 5 2.D
    unimplemented!("sock_recv")
}

struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
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
