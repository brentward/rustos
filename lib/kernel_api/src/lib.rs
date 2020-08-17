#![feature(asm)]
// #![feature(specialization)]
// #![feature(toowned_clone_into)]
// #![feature(mem_take)]
// #![feature(ptr_cast)]
// #![feature(const_raw_ptr_deref)]
#![no_std]

extern crate alloc;

use core::fmt;
use core::str;
use shim::io;

pub mod fs;
pub mod network;
// pub mod args;
// pub mod cstr;

#[cfg(feature = "user-space")]
pub mod syscall;

pub type OsResult<T> = core::result::Result<T, OsError>;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OsError {
    Unknown = 0,
    Ok = 1,
    Utf8Error = 2,
    FmtError = 3,
    InvalidPid = 4,
    MaxPidExceeded = 5,

    NoEntry = 10,
    NoMemory = 20,
    NoVmSpace = 30,
    NoAccess = 40,
    BadAddress = 50,
    FileExists = 60,
    InvalidArgument = 70,
    NotAFile = 80,
    NotADir = 90,

    IoError = 101,
    IoErrorEof = 102,
    IoErrorInvalidData = 103,
    IoErrorInvalidInput = 104,
    IoErrorTimedOut = 105,

    InvalidSocket = 200,
    IllegalSocketOperation = 201,

    // CStringError = 301,
    // NullError = 302,
    // FromBytesWithNulError = 303,
    // FromVecWithNulError = 304,
    // IntoStringError = 305


}

impl core::convert::From<u64> for OsError {
    fn from(e: u64) -> Self {
        match e {
            1 => OsError::Ok,
            2 => OsError::Utf8Error,
            3 => OsError::FmtError,
            4 => OsError::InvalidPid,
            5 => OsError::MaxPidExceeded,

            10 => OsError::NoEntry,
            20 => OsError::NoMemory,
            30 => OsError::NoVmSpace,
            40 => OsError::NoAccess,
            50 => OsError::BadAddress,
            60 => OsError::FileExists,
            70 => OsError::InvalidArgument,
            80 => OsError::NotAFile,
            90 => OsError::NotADir,

            101 => OsError::IoError,
            102 => OsError::IoErrorEof,
            103 => OsError::IoErrorInvalidData,
            104 => OsError::IoErrorInvalidInput,

            200 => OsError::InvalidSocket,
            201 => OsError::IllegalSocketOperation,

            // 301 => OsError::CStringError,
            // 302 => OsError::NullError,
            // 303 => OsError::FromBytesWithNulError,
            // 304 => OsError::FromVecWithNulError,
            // 305 => OsError::IntoStringError,


            _ => OsError::Unknown,
        }
    }
}

impl core::convert::From<io::Error> for OsError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::UnexpectedEof => OsError::IoErrorEof,
            io::ErrorKind::InvalidData => OsError::IoErrorInvalidData,
            io::ErrorKind::InvalidInput => OsError::IoErrorInvalidInput,
            io::ErrorKind::TimedOut => OsError::IoErrorTimedOut,
            io::ErrorKind::NotFound => OsError::NoEntry,
            _ => OsError::IoError,
        }
    }
}

impl core::convert::From<fmt::Error> for OsError {
    fn from(_e: fmt::Error) -> Self {
        OsError::FmtError
    }
}

impl core::convert::From<OsError> for fmt::Error {
    fn from(_e: OsError) -> Self {
        fmt::Error
    }
}

impl core::convert::From<str::Utf8Error> for OsError {
    fn from(_e: str::Utf8Error) -> Self {
        OsError::Utf8Error
    }
}

// impl core::convert::From<cstr::NulError> for OsError {
//     fn from(_e: cstr::NulError) -> Self {
//         OsError::CStringError
//     }
// }
//
// impl core::convert::From<cstr::FromBytesWithNulError> for OsError {
//     fn from(_e: cstr::FromBytesWithNulError) -> Self {
//         OsError::CStringError
//     }
// }
//
// impl core::convert::From<cstr::FromVecWithNulError> for OsError {
//     fn from(_e: cstr::FromVecWithNulError) -> Self {
//         OsError::CStringError
//     }
// }
//
// impl core::convert::From<cstr::IntoStringError> for OsError {
//     fn from(_e: cstr::IntoStringError) -> Self {
//         OsError::CStringError
//     }
// }

// NullError = 302,
// FromBytesWithNulError = 303,
// FromVecWithNulError = 304,
// IntoStringError = 305


// #[derive(Clone, Copy, Debug)]
// pub struct SocketDescriptor(u64);
//
// impl SocketDescriptor {
//     pub fn raw(&self) -> u64 {
//         self.0
//     }
// }
//
// #[derive(Debug)]
// pub struct SocketStatus {
//     pub is_active: bool,
//     pub is_listening: bool,
//     pub can_send: bool,
//     pub can_recv: bool,
// }
//
// pub struct IpAddr {
//     pub ip: u32,
//     pub port: u16,
// }
//
// impl IpAddr {
//     pub fn new((ip1, ip2, ip3, ip4): (u8, u8, u8, u8), port: u16) -> Self {
//         IpAddr {
//             ip: u32::from_be_bytes([ip1, ip2, ip3, ip4]),
//             port,
//         }
//     }
// }
//
// impl fmt::Debug for IpAddr {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let bytes = self.ip.to_be_bytes();
//         write!(
//             f,
//             "IpAddr({}.{}.{}.{}:{})",
//             bytes[0], bytes[1], bytes[2], bytes[3], self.port
//         )
//     }
// }

pub const NR_SLEEP: usize = 1;
pub const NR_TIME: usize = 2;
pub const NR_EXIT: usize = 3;
pub const NR_WRITE: usize = 4;
pub const NR_GETPID: usize = 5;
pub const NR_WRITE_STR: usize = 6;
pub const NR_SBRK: usize = 7;
pub const NR_RAND: usize = 8;
pub const NR_RRAND: usize = 9;
pub const NR_ENTROPY: usize = 10;
pub const NR_FORK: usize = 11;
pub const NR_START_P: usize = 12;
pub const NR_BRK: usize = 13;
pub const NR_ARGS_COUNT: usize = 14;
pub const NR_READ_ARG: usize = 15;
pub const NR_PUSH_ARG: usize = 16;
pub const NR_LOAD_P: usize = 17;
pub const NR_PID_IS_ALIVE: usize = 18;

pub const NR_SOCK_CREATE: usize = 20;
pub const NR_SOCK_STATUS: usize = 21;
pub const NR_SOCK_CONNECT: usize = 22;
pub const NR_SOCK_LISTEN: usize = 23;
pub const NR_SOCK_SEND: usize = 24;
pub const NR_SOCK_RECV: usize = 25;

pub const NR_OPEN: usize = 30;
pub const NR_READ: usize = 31;
pub const NR_GETDENTS: usize = 32;
pub const NR_STAT: usize = 33;
pub const NR_GETCWD: usize = 34;
pub const NR_CHDIR: usize = 35;
