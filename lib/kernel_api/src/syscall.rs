use core::fmt;
use core::fmt::Write;
use core::time::Duration;
use shim::path::Path;

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
             : "x7"
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
             : "x0", "x7"
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

pub fn rand(min: u32, max: u32) -> u32 {
    let mut _ecode: u64;
    let mut rand: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(rand), "=r"(_ecode)
             : "r"(min as u64), "r"(max as u64), "i"(NR_RAND)
             : "x0", "x7"
             : "volatile");
    }
    rand as u32
}

pub fn rrand() -> u32 {
    let mut _ecode: u64;
    let mut rrand: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(rrand), "=r"(_ecode)
             : "i"(NR_RAND)
             : "x0", "x7"
             : "volatile");
    }
    rrand as u32
}

pub fn entropy() -> u32 {
    let mut _ecode: u64;
    let mut entropy: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(entropy), "=r"(_ecode)
             : "i"(NR_RAND)
             : "x0", "x7"
             : "volatile");
    }
    entropy as u32
}

pub fn sock_create() -> SocketDescriptor {
    let mut _ecode: u64;
    let mut sid: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(sid), "=r"(_ecode)
             : "i"(NR_SOCK_CREATE)
             : "x0", "x7"
             : "volatile");
    }

    SocketDescriptor::from(sid)
}

pub fn sock_status(descriptor: SocketDescriptor) -> OsResult<SocketStatus> {
    let mut ecode: u64;
    let mut is_active: bool;
    let mut is_listening: bool;
    let mut can_send: bool;
    let mut can_recv: bool;

    unsafe {
        asm!("mov x0, $5
              svc $6
              mov $0, x0
              mov $1, x1
              mov $2, x2
              mov $3, x3
              mov $4, x7"
             : "=r"(is_active), "=r"(is_listening), "=r"(can_send), "=r"(can_recv), "=r"(ecode)
             : "r"(descriptor.raw()), "i"(NR_SOCK_STATUS)
             : "x0", "x1", "x2", "x3", "x7"
             : "volatile");
    }

    err_or!(ecode,  SocketStatus { is_active, is_listening, can_send, can_recv })
}

pub fn sock_connect(descriptor: SocketDescriptor, addr: IpAddr) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              mov x2, $3
              svc $4
              mov $0, x7"
             : "=r"(ecode)
             : "r"(descriptor.raw()), "r"(addr.ip), "r"(addr.port), "i"(NR_SOCK_CONNECT)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
}

pub fn sock_listen(descriptor: SocketDescriptor, local_port: u16) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              svc $3
              mov $0, x7"
             : "=r"(ecode)
             : "r"(descriptor.raw()), "r"(local_port), "i"(NR_SOCK_LISTEN)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
}

pub fn sock_send(descriptor: SocketDescriptor, buf: &[u8]) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut bytes: usize;
    let len = buf.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(bytes), "=r"(ecode)
             : "r"(descriptor.raw()), "r"(buf_ptr), "r"(len), "i"(NR_SOCK_SEND)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn sock_recv(descriptor: SocketDescriptor, buf: &mut [u8]) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut bytes: usize;
    let len = buf.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(bytes), "=r"(ecode)
             : "r"(descriptor.raw()), "r"(buf_ptr), "r"(len), "i"(NR_SOCK_RECV)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn open<P: AsRef<Path>>(path: P) -> OsResult<u64> {
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
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

pub fn getdents<P: AsRef<Path>>(path: P, buf: &mut [fs::DirEnt], offset: u64) -> OsResult<u64> {
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
    let buf_ptr = buf.as_ptr() as u64;
    let buf_len = buf.len();
    let mut ecode: u64;
    let mut entries: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              mov x3, $5
              mov x4, $6
              svc $7
              mov $0, x0
              mov $1, x7"
             : "=r"(entries), "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "r"(buf_ptr), "r"(buf_len), "r"(offset), "i"(NR_GETDENTS)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, entries)
}

pub fn stat<P: AsRef<Path>>(path: P, buf: &mut [fs::Stat]) -> OsResult<()> {
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              mov x2, $3
              svc $4
              mov $0, x7"
             : "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "r"(buf_ptr), "i"(NR_STAT)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
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
