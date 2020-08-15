use core::fmt;
use core::fmt::Write;
use core::time::Duration;
use shim::path::{Path, PathBuf};
use alloc::string::String;
use alloc::vec::Vec;

use crate::*;
use crate::fs::{Handle, HandleDescriptor, ProcessDescriptor};
use crate::network::{SocketStatus, IpAddr};
// use crate::args::CArgs;
// use crate::cstr::CString;

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

pub fn write(handle: &Handle, buf: &[u8]) -> OsResult<usize> {
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
             : "r"(handle.raw()), "r"(buf_ptr), "r"(len), "i"(NR_WRITE)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn write_str(handle: &Handle, msg: &str) -> OsResult<usize> {
// pub fn write_str(handle: &Handle, msg: &str) {
    let msg_ptr = msg.as_ptr() as u64;
    let msg_len = msg.len() as u64;
    let mut ecode: u64;
    let mut len: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(len), "=r"(ecode)
             : "r"(handle.raw()), "r"(msg_ptr), "r"(msg_len), "i"(NR_WRITE_STR)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, len as usize)
}

pub fn getpid() -> ProcessDescriptor {
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

    ProcessDescriptor::from(pid)
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

pub fn brk(ptr: usize) -> OsResult<()> {
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              svc $2
              mov $0, x7"
             : "=r"(ecode)
             : "r"(ptr as u64), "i"(NR_BRK)
             : "x7"
             : "volatile");
    }
    err_or!(ecode, ())
}

fn args_count() -> usize {
    let mut _ecode: u64;
    let mut count: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(count), "=r"(_ecode)
             : "i"(NR_ARGS_COUNT)
             : "x0", "x7"
             : "volatile");
    }

    count as usize
}

fn read_arg(idx: usize, buf: &mut [u8], offset: usize) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut bytes: usize;
    let len = buf.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              mov x3, $5
              svc $6
              mov $0, x0
              mov $1, x7"
             : "=r"(bytes), "=r"(ecode)
             : "r"(idx as u64), "r"(buf_ptr), "r"(len), "r"(offset as u64) "i"(NR_READ_ARG)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

fn push_arg(pid: ProcessDescriptor, arg: &str) -> OsResult<()> {
    // println!("push_arg() pid: {}, arg: {}", pid, arg);
    let arg_ptr = arg.as_ptr() as u64;
    let arg_len = arg.len() as u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              mov x2, $3
              svc $4
              mov $0, x7"
             : "=r"(ecode)
             : "r"(pid.raw()), "r"(arg_ptr), "r"(arg_len), "i"(NR_PUSH_ARG)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())

}

pub fn args() -> Vec<String> {
    let mut args_v = Vec::new();
    let args_count = args_count();
    // println!("args() arg_count: {}", args_count);
    for idx in 0..args_count {
        // println!("creating arg: {}", idx);
        let mut arg_v = Vec::new();
        let mut buf = [0; 64];
        let mut bytes_total = 0;
        let mut bytes_read = 0;
        loop {
            bytes_read = read_arg(idx, &mut buf, bytes_total).unwrap();
            // println!("arg: {}, read: {}", idx, bytes_read);
            if bytes_read == 0 {
                break
            }
            bytes_total += bytes_read;

            use crate::io::Write;

            let _bytes_written = arg_v.write(&buf)
                .unwrap();
        }
        // println!("arg: {}, total: {}", idx, bytes_total);
        while arg_v.len() > bytes_total {
            // println!("pop on arg {}", idx);
            arg_v.pop();
        }
        let arg = String::from_utf8(arg_v).unwrap();
        // println!("arg: {} is {}", idx, arg);
        args_v.push(arg);
    }
    args_v
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

pub fn sock_create() -> Handle {
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
    Handle::Socket(HandleDescriptor::from(sid))
}

pub fn sock_status(handle: &Handle) -> OsResult<SocketStatus> {
    match handle {
        Handle::Socket(_) => (),
        _ => return Err(OsError::InvalidSocket),
    };
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
             : "r"(handle.raw()), "i"(NR_SOCK_STATUS)
             : "x0", "x1", "x2", "x3", "x7"
             : "volatile");
    }

    err_or!(ecode,  SocketStatus { is_active, is_listening, can_send, can_recv })
}

pub fn sock_connect(handle: &Handle, addr: IpAddr) -> OsResult<()> {
    match handle {
        Handle::Socket(_) => (),
        _ => return Err(OsError::InvalidSocket),
    };

    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              mov x2, $3
              svc $4
              mov $0, x7"
             : "=r"(ecode)
             : "r"(handle.raw()), "r"(addr.ip), "r"(addr.port), "i"(NR_SOCK_CONNECT)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
}

pub fn sock_listen(handle: &Handle, local_port: u16) -> OsResult<()> {
    match handle {
        Handle::Socket(_) => (),
        _ => return Err(OsError::InvalidSocket),
    };

    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              svc $3
              mov $0, x7"
             : "=r"(ecode)
             : "r"(handle.raw()), "r"(local_port), "i"(NR_SOCK_LISTEN)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
}

pub fn sock_send(handle: &Handle, buf: &[u8]) -> OsResult<usize> {
    match handle {
        Handle::Socket(_) => (),
        _ => return Err(OsError::InvalidSocket),
    };

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
             : "r"(handle.raw()), "r"(buf_ptr), "r"(len), "i"(NR_SOCK_SEND)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn sock_recv(handle: &Handle, buf: &mut [u8]) -> OsResult<usize> {
    match handle {
        Handle::Socket(_) => (),
        _ => return Err(OsError::InvalidSocket),
    };

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
             : "r"(handle.raw()), "r"(buf_ptr), "r"(len), "i"(NR_SOCK_RECV)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn open<P: AsRef<Path>>(path: P) -> OsResult<Handle> {
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
    let mut ecode: u64;
    let mut handle_idx: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(handle_idx), "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "i"(NR_OPEN)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, Handle::File(HandleDescriptor::from(handle_idx)))
}

pub fn read(handle: &Handle, buf: &mut [u8]) -> OsResult<usize> {
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
             : "r"(handle.raw()), "r"(buf_ptr), "r"(count), "i"(NR_READ)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

fn k_getdents<P: AsRef<Path>>(path: P, buf: &mut [fs::DirEnt], offset: u64) -> OsResult<u64> {
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

pub fn getdents<P: AsRef<Path>>(path: P) ->Vec<fs::DirEnt> {

    let mut dent_buf = [fs::DirEnt::default(); 16];
    let mut dents_v = Vec::<fs::DirEnt>::new();
    let mut dents_total = 0u64;

    loop {
        // let path: Path = path.as_ref().clone();
        let dents = match k_getdents(&path, &mut dent_buf, dents_total) {
            Ok(dents) => dents,
            Err(e) => {
                println!("getdetns() error: {:?}", e);
                return dents_v
            }
        };
        for dent in dent_buf[0..dents as usize].iter() {
            dents_v.push(*dent);
        }
        dents_total += dents;
        if dents == 0 {
            break
        }
    }
    dents_v

}

fn k_stat<P: AsRef<Path>>(path: P, buf: &mut [fs::Stat]) -> OsResult<()> {
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

pub fn stat<P: AsRef<Path>>(path: P) -> OsResult<fs::Stat> {
    let mut stat_buf = [fs::Stat::default()];
    k_stat(path, &mut stat_buf)?;
    Ok(stat_buf[0])
}

pub fn chdir<P: AsRef<Path>>(path: P) -> OsResult<()> {
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              mov x1, $2
              svc $3
              mov $1, x7"
             : "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "i"(NR_CHDIR)
             : "x7"
             : "volatile");
    }

    err_or!(ecode, ())
}

fn k_getcwd(buf: &mut [u8], offset: usize) -> OsResult<usize> {
    let buf_ptr = buf.as_ptr() as u64;
    let mut ecode: u64;
    let mut bytes: usize;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $5
              mov $0, x0
              mov $1, x7"
             : "=r"(bytes), "=r"(ecode)
             : "r"(buf_ptr), "r"(buf.len()), "r"(offset) "i"(NR_GETCWD)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, bytes)
}

pub fn getcwd() -> PathBuf {
    use shim::io::Write;

    let mut cwd_bytes = 0;
    let mut cwd_v = Vec::<u8>::new();
    loop {
        let mut cwd_buf = [0u8; 512];
        let bytes = match k_getcwd(&mut cwd_buf, cwd_bytes) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("getcwd() error: {:?}", e);
                return PathBuf::from("")
            },
        };
        if bytes == 0 {
            break
        }
        cwd_bytes += bytes;
        match cwd_v.write(&cwd_buf[..bytes]) {
            Ok(_) => (),
            Err(e) => {
                println!("getcwd() error: {:?}", e);
                return PathBuf::from("")
            }
        };
    }
    PathBuf::from(String::from_utf8(cwd_v).unwrap())

}

fn load_p<P: AsRef<Path>>(path: P) -> OsResult<ProcessDescriptor> {
    // println!("load_p() path: {:?}", path.as_ref());
    let path: &Path = path.as_ref();
    let path_str = match path.to_str() {
        Some(str) => str,
        None => return Err(OsError::InvalidArgument),
    };
    let path_ptr = path_str.as_ptr() as u64;
    let path_len = path_str.len();
    let mut pid: u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(pid), "=r"(ecode)
             : "r"(path_ptr), "r"(path_len), "i"(NR_LOAD_P)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, ProcessDescriptor::from(pid))
}

fn start_p(pid: ProcessDescriptor) -> OsResult<()> {
    // println!("start_p() pid: {}", pid);
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              svc $2
              mov $0, x7"
             : "=r"(ecode)
             : "r"(pid.raw()), "i"(NR_START_P)
             : "x7"
             : "volatile");
    }
    err_or!(ecode, ())
}

pub fn wait(pid: ProcessDescriptor) -> OsResult<()> {
    // println!("wait() pid: {}", pid);
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              svc $2
              mov $0, x7"
             : "=r"(ecode)
             : "r"(pid.raw()), "i"(NR_WAIT)
             : "x7"
             : "volatile");
    }
    err_or!(ecode, ())
}


pub fn execve<P: AsRef<Path>>(path: P, args: &Vec<String>) -> OsResult<ProcessDescriptor> {
    let pid = load_p(path)?;
    for arg in args {
        // println!("execev() arg: {}", arg);
        push_arg(pid, arg.as_str())?;
    }
    // println!("execev() starting  pid: {}", pid);
    start_p(pid)?;
    // println!("execev() returning pid: {}", pid);
    Ok(pid)
}


struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(&Handle::StdOut, s)?;
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
