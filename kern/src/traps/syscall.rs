use alloc::boxed::Box;
use core::time::Duration;
use shim::path::PathBuf;
use core::mem::size_of;
use core::ops::Add;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::{kprint, CONSOLE};
use crate::param::USER_IMG_BASE;
use crate::process::{State, FdEntry};
use crate::traps::TrapFrame;
use crate::{ETHERNET, SCHEDULER, FILESYSTEM};
use crate::vm::{VirtualAddr, Page, PagePerm};

use kernel_api::*;
use pi::timer;
use fat32::traits::{FileSystem, Entry, Dir};

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let start_time = timer::current_time();
    let end_time = start_time + Duration::from_millis(ms as u64);
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        let current_time = timer::current_time();
        if current_time >= end_time {
            p.context.x[0] = (current_time - start_time).as_millis() as u64;
            p.context.x[7] = OsError::Ok as u64;
            true
        } else {
            false
        }
    })), tf);
}

/// Returns current time.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns two
/// parameter:
///  - current time as seconds
///  - fractional part of the current time, in nanoseconds.
pub fn sys_time(tf: &mut TrapFrame) {
    let current_time = timer::current_time();
    let seconds = current_time.as_secs();
    let nanoseconds = (current_time - Duration::from_secs(seconds)).as_nanos() as u64;
    tf.x[0] = seconds;
    tf.x[1] = nanoseconds;
    tf.x[7] = OsError::Ok as u64;
}

/// Kills the current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    let _pid_option = SCHEDULER.kill(tf);
}

/// Writes to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    if b.is_ascii() {
        let ch = b as char;
        kprint!("{}", ch);
        tf.x[7] = OsError::Ok as u64;
    } else {
        tf.x[7] = OsError::IoErrorInvalidInput as u64;
    }
}

/// Returns the current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.x[0] = tf.tpidr;
    tf.x[7] = OsError::Ok as u64;
}

pub fn sys_sbrk(size: usize, tf: &mut TrapFrame)  {
    SCHEDULER.critical(|scheduler| {
        let mut process = scheduler.find_process(tf);
        let next_heap_ptr = process.heap_ptr.add(VirtualAddr::from(size));
        while process.heap_page.add(VirtualAddr::from(Page::SIZE)).as_usize() < next_heap_ptr.as_usize() {
            let next_heap_page = process.heap_page.add(VirtualAddr::from(Page::SIZE));
            if next_heap_page.as_usize() >= process.stack_base.as_usize() {
                tf.x[7] = OsError::NoVmSpace as u64;
                return;
            }
            let _heap_page = process.vmap.alloc(next_heap_page, PagePerm::RW);
            process.heap_page = next_heap_page;
        }
        process.heap_ptr = next_heap_ptr;
        tf.x[0] = process.heap_ptr.as_u64();
        tf.x[7] = OsError::Ok as u64;
    });
}

pub fn sys_rand(min: u32, max: u32, tf: &mut TrapFrame) {
    let rand = {
        let mut rng = crate::rng::RNG.lock();
        rng.rand(min, max)
    };
    tf.x[0] = rand as u64;
    tf.x[7] = OsError::Ok as u64;
}

pub fn sys_rrand(tf: &mut TrapFrame) {
    let rrand = {
        let mut rng = crate::rng::RNG.lock();
        rng.r_rand()
    };
    tf.x[0] = rrand as u64;
    tf.x[7] = OsError::Ok as u64;
}

pub fn sys_entropy(tf: &mut TrapFrame) {
    let entropy = {
        let mut rng = crate::rng::RNG.lock();
        rng.entropy()
    };
    tf.x[0] = entropy as u64;
    tf.x[7] = OsError::Ok as u64;
}

pub fn sys_open(va: usize, len: usize, tf: &mut TrapFrame) {
    use crate::console::kprintln;

    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));


    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        // let path_slice = unsafe { match p.vmap
        //     .get_slice_at_va(VirtualAddr::from(path_ptr), path_len) {
        //     Ok(slice) => slice,
        //     Err(_) => {
        //         p.context.x[7] = 104;
        //         return true
        //     }
        // }};
        // let result = unsafe { to_user_slice(va, len) }
        //     .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));
        // let path = match str::from_utf8(path_slice) {
        //     Ok(path) => path,
        //     Err(_e) => {
        //         p.context.x[7] = 50;
        //         return true
        //     }
        // };
        match result {
            Ok(path) => {
                let path_buf = PathBuf::from(path);

                let entry = match FILESYSTEM.open(path_buf.as_path()) {
                    Ok(entry) => entry,
                    Err(_) => {
                        p.context.x[7] = OsError::NoEntry as u64;
                        return true
                    }
                };
                if p.unused_file_descriptors.len() > 0 {
                    let fd = p.unused_file_descriptors.pop()
                        .expect("Unexpected p.unused_file_descriptors.pop() failed after len check");
                    match p.file_table[fd] {
                        Some(_) => {
                            p.context.x[7] = OsError::IoErrorInvalidData as u64;
                            true
                        }
                        None => {
                            match entry.is_file() {
                                true => {
                                    let file = entry.into_file()
                                        .expect("Entry unexpectedly failed to convert to file");
                                    p.file_table[fd] = Some(FdEntry::File(Box::new(file)));
                                }
                                false => {
                                    let dir = entry.into_dir()
                                        .expect("Entry unexpectedly failed to convert to dir");
                                    let dir_entries = dir.entries().unwrap(); //FIXME
                                    p.file_table[fd] = Some(FdEntry::DirEntries(Box::new(dir_entries)));
                                }
                            }
                            p.context.x[0] = fd as u64;
                            p.context.x[7] = OsError::Ok as u64;
                            true
                        }
                    }
                } else {
                    let fd = p.file_table.len();
                    match entry.is_file() {
                        true => {
                            let file = entry.into_file()
                                .expect("Entry unexpectedly failed to convert to file");
                            p.file_table.push(Some(FdEntry::File(Box::new(file))));
                        }
                        false => {
                            let dir = entry.into_dir()
                                .expect("Entry unexpectedly failed to convert to dir");
                            let dir_entries = dir.entries().unwrap(); //FIXME
                            p.file_table.push(Some(FdEntry::DirEntries(Box::new(dir_entries))));
                        }
                    }
                    p.context.x[0] = fd as u64;
                    p.context.x[7] = OsError::Ok as u64;
                    true
                }
            }
            Err(e) => {
                p.context.x[7] = e as u64;
                true
            }
        }
    })), tf);
}

pub fn sys_read(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    use crate::console::kprintln;
    use shim::io::Read;


    SCHEDULER.switch(State::Waiting(Box::new(move |p| {


        // let mut buf_slice = unsafe { match p.vmap
        //     .get_mut_slice_at_va(VirtualAddr::from(va), len) {
        //     Ok(slice) => slice,
        //     Err(_) => {
        //         p.context.x[7] = 104;
        //         return true
        //     }
        // }};

        let result = unsafe { to_user_slice_mut(va, len) }
            .map_err(|_| OsError::InvalidArgument);

        match result {
            Ok(buf_slice) => {
                match p.file_table.remove(fd) {
                    Some(entry) => {
                        match entry {
                            FdEntry::Console => {
                                let byte =  CONSOLE.lock().read_byte();
                                p.file_table.insert(fd, Some(FdEntry::Console));
                                buf_slice[0] = byte;
                                p.context.x[0] = 1;
                                p.context.x[7] = OsError::Ok as u64;
                                true

                            }
                            FdEntry::File(mut file) => {
                                let bytes = match file.read(&mut buf_slice[..]) {
                                    Ok(bytes) => bytes,
                                    Err(_) => {
                                        p.context.x[7] = OsError::IoError as u64;
                                        return true
                                    }
                                };
                                p.file_table.insert(fd, Some(FdEntry::File(file)));
                                p.context.x[0] = bytes as u64;
                                p.context.x[7] = OsError::Ok as u64;
                                true

                            }
                            FdEntry::DirEntries(dir_entries) => {
                                p.file_table.insert(fd, Some(FdEntry::DirEntries(dir_entries)));
                                p.context.x[7] = 80;
                                true
                            }
                        }
                    }
                    None => {
                        p.context.x[7] = 10;
                        true
                    }
                }

            }
            Err(e) => {
                p.context.x[7] = e as u64;
                true
            }
        }

    })), tf);
}

// pub fn sys_getdent(fd: usize, va: usize, len: usize, tf: &mut TrapFrame) {
//     SCHEDULER.switch(State::Waiting(Box::new(move |p| {
//         let overflow = va.checked_add(len * size_of::<fs::DirEnt>()).is_none();
//         let result = if va >= USER_IMG_BASE && !overflow {
//             Ok(va)
//         } else {
//             Err(OsError::BadAddress)
//         };
//
//         match &result {
//             Ok(va) => {
//                 let mut entries = 0u64;
//                 let mut dir_entries = match p.file_table.remove(fd) {
//                     Some(entry) => {
//                         match entry {
//                             FdEntry::Console => {
//                                 p.file_table.insert(fd, Some(FdEntry::Console));
//                                 p.context.x[7] = OsError::Ok as u64;
//                                 return true
//                             }
//                             FdEntry::File(file) => {
//                                 p.file_table.insert(fd, Some(FdEntry::File(file)));
//                                 p.context.x[7] = OsError::Ok as u64;
//                                 return true
//                             }
//                             FdEntry::DirEntries(dir_entries) => dir_entries
//                         }
//                     }
//                     None => {
//                         p.context.x[7] = 10;
//                         return true
//                     }
//                 };
//                 for index in 0..len {
//                     match dir_entries.next() {
//                         Some(entry) => {
//                             let dent_va = va.add(size_of::<fs::DirEnt>() * index);
//                             let mut dent = unsafe { &mut *(dent_va as  *mut fs::DirEnt) };
//
//                             dent.set_name(entry.name());
//                             match entry.is_file() {
//                                 true => dent.set_d_type(fs::DirType::File),
//                                 false => dent.set_d_type(fs::DirType::Dir),
//                             }
//                             entries += 1;
//
//                         }
//                         None => {
//                             break
//                         }
//                     }
//                 }
//                 p.file_table.insert(fd, Some(FdEntry::DirEntries(dir_entries)));
//                 p.context.x[0] = entries;
//                 p.context.x[7] = OsError::Ok as u64;
//
//                 true
//             }
//             Err(e) => {
//                 tf.x[7] = *e as u64;
//                 true
//             },
//         }
//     }
//     )), tf);
// }

/// Creates a socket and saves the socket handle in the current process's
/// socket list.
///
/// This function does neither take any parameter nor return anything,
/// except the usual return code that indicates successful syscall execution.
pub fn sys_sock_create(tf: &mut TrapFrame) {
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        let sock_idx = process.sockets.len() + 3;
        let mut handle = ETHERNET.add_socket();
        process.sockets.push(handle);
        tf.x[0] = sock_idx as u64;
        tf.x[7] = OsError::Ok as u64;
    });
}

/// Returns the status of a socket.
///
/// This system call takes a socket descriptor as the first parameter.
///
/// In addition to the usual status value, this system call returns four boolean
/// values that describes the status of the queried socket.
///
/// - x0: is_active
/// - x1: is_listening
/// - x2: can_send
/// - x3: can_recv
///
/// # Errors
/// This function returns `OsError::InvalidSocket` if a socket that corresponds
/// to the provided descriptor is not found.
pub fn sys_sock_status(sock_idx: usize, tf: &mut TrapFrame) {
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        match process.sockets.get(sock_idx - 3) {
            Some(handle) => {
                let (is_active, is_listening, can_send, can_recv) = ETHERNET.with_socket(*handle, |socket| {
                    (socket.is_active(),  socket.is_listening(), socket.can_send(), socket.can_recv())
                });
                tf.x[0] = is_active as u64;
                tf.x[1] = is_listening as u64;
                tf.x[2] = can_send as u64;
                tf.x[3] = can_recv as u64;
                tf.x[7] = OsError::Ok as u64;
            }
            None => {
                tf.x[7] = OsError::InvalidSocket as u64;
            }
        };
    });
}

/// Connects a local ephemeral port to a remote IP endpoint with a socket.
///
/// This system call takes a socket descriptor as the first parameter, the IP
/// of the remote endpoint as the second paramter in big endian, and the port
/// number of the remote endpoint as the third parameter.
///
/// `handle_syscall` should read the value of registers and create a struct that
/// implements `Into<IpEndpoint>` when calling this function.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::NoEntry`: Fails to allocate an ephemeral port
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `connect()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `connect()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `connect()`.
pub fn sys_sock_connect(
    sock_idx: usize,
    remote_endpoint: impl Into<IpEndpoint>,
    tf: &mut TrapFrame,
) {
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        match process.sockets.get(sock_idx - 3) {
            Some(handle) => {
                let port: u16;
                match ETHERNET.get_ephemeral_port() {
                    Some(p) => port = p,
                    None => tf.x[7] = {
                        OsError::NoEntry as u64;
                        return;
                    }
                };
                match ETHERNET.mark_port(port) {
                    Some(_) => (),
                    None => tf.x[7] = {
                        OsError::NoEntry as u64;
                        return;
                    }
                };
                ETHERNET.with_socket(*handle, |socket| {
                    match socket.connect(remote_endpoint, port) {
                        Ok(()) => tf.x[7] = OsError::Ok as u64,
                        Err(smoltcp::Error::Illegal) => tf.x[7] = OsError::IllegalSocketOperation as u64,
                        Err(smoltcp::Error::Unaddressable) => tf.x[7] = OsError::BadAddress as u64,
                        Err(_) => tf.x[7] = OsError::Unknown as u64,
                    }
                })
            }
            None => tf.x[7] = OsError::InvalidSocket as u64,
        };
    });
}

/// Listens on a local port for an inbound connection.
///
/// This system call takes a socket descriptor as the first parameter and the
/// local ports to listen on as the second parameter.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `listen()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `listen()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `listen()`.
pub fn sys_sock_listen(sock_idx: usize, local_port: u16, tf: &mut TrapFrame) {
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        match process.sockets.get(sock_idx - 3) {
            Some(handle) => {
                match ETHERNET.mark_port(local_port) {
                    Some(_) => (),
                    None => {
                        tf.x[7] = OsError::NoEntry as u64;
                        return
                    }
                };
                ETHERNET.with_socket(*handle, |socket| {
                    match socket.listen(local_port) {
                        Ok(()) => tf.x[7] = OsError::Ok as u64,
                        Err(smoltcp::Error::Illegal) => tf.x[7] = OsError::IllegalSocketOperation as u64,
                        Err(smoltcp::Error::Unaddressable) => tf.x[7] = OsError::BadAddress as u64,
                        Err(_) => tf.x[7] = OsError::Unknown as u64,
                    }
                });
            }
            None => tf.x[7] = OsError::InvalidSocket as u64,
        };
    });
}

/// Returns a slice from a virtual address and a legnth.
///
/// # Errors
/// This functions returns `Err(OsError::BadAddress)` if the slice is not entirely
/// in userspace.
unsafe fn to_user_slice<'a>(va: usize, len: usize) -> OsResult<&'a [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts(va as *const u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}

/// Returns a mutable slice from a virtual address and a legnth.
///
/// # Errors
/// This functions returns `Err(OsError::BadAddress)` if the slice is not entirely
/// in userspace.
unsafe fn to_user_slice_mut<'a>(va: usize, len: usize) -> OsResult<&'a mut [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts_mut(va as *mut u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}

/// Sends data with a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes sent.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `send_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_send(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    match unsafe { to_user_slice(va, len) } {
        Ok(data) => {
            SCHEDULER.critical(|scheduler|{
                let mut process = scheduler.find_process(tf);
                match process.sockets.get(sock_idx - 3) {
                    Some(handle) => {
                        ETHERNET.with_socket(*handle, |socket| {
                            match socket.send_slice(data) {
                                Ok(bytes) => {
                                    tf.x[0] = bytes as u64;
                                    tf.x[7] = OsError::Ok as u64;
                                }
                                Err(smoltcp::Error::Illegal) => tf.x[7] = OsError::IllegalSocketOperation as u64,
                                Err(_) => tf.x[7] = OsError::Unknown as u64,
                            }
                        });

                    }
                    None => tf.x[7] = OsError::InvalidSocket as u64,
                };
            });
        }
        Err(e) => tf.x[7] = e as u64,
    }
}

/// Receives data from a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes read.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `recv_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_recv(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    match unsafe { to_user_slice_mut(va, len) } {
        Ok(data) => {
            SCHEDULER.critical(|scheduler|{
                let mut process = scheduler.find_process(tf);
                match process.sockets.get(sock_idx - 3) {
                    Some(handle) => {
                        ETHERNET.with_socket(*handle, |socket| {
                            match socket.recv_slice(data) {
                                Ok(bytes) => {
                                    tf.x[0] = bytes as u64;
                                    tf.x[7] = OsError::Ok as u64;
                                }
                                Err(smoltcp::Error::Illegal) => tf.x[7] = OsError::IllegalSocketOperation as u64,
                                Err(_) => tf.x[7] = OsError::Unknown as u64,
                            }
                        });

                    }
                    None => tf.x[7] = OsError::InvalidSocket as u64,
                };
            });
        }
        Err(e) => tf.x[7] = e as u64,
    }
}

/// Writes a UTF-8 string to the console.
///
/// This system call takes the address of the buffer as the first parameter and
/// the length of the buffer as the second parameter.
///
/// In addition to the usual status value, this system call returns the length
/// of the UTF-8 message.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::InvalidArgument`: The provided buffer is not UTF-8 encoded.
pub fn sys_write_str(va: usize, len: usize, tf: &mut TrapFrame) {
    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));

    match result {
        Ok(msg) => {
            kprint!("{}", msg);

            tf.x[0] = msg.len() as u64;
            tf.x[7] = OsError::Ok as u64;
        }
        Err(e) => {
            tf.x[7] = e as u64;
        }
    }
}

struct IpAddr {
    pub ip: u32,
    pub port: u16,
}

impl IpAddr {
    fn from(ip_bytes: u64, port_bytes: u64) -> IpAddr {
        IpAddr {
            ip: ip_bytes as u32,
            port: port_bytes as u16,
        }
    }
}

impl Into<IpEndpoint> for IpAddr {
    fn into(self) -> IpEndpoint {
        let bytes = self.ip.to_be_bytes();
        IpEndpoint::new(IpAddress::v4(bytes[0], bytes[1], bytes[2], bytes[3]), self.port)
    }
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    match num {
        1 => sys_sleep(tf.x[0] as u32, tf),
        2 => sys_time(tf),
        3 => sys_exit(tf),
        4 => sys_write(tf.x[0] as u8, tf),
        5 => sys_getpid(tf),
        6 => sys_write_str(tf.x[0] as usize, tf.x[1] as usize, tf),
        7 => sys_sbrk(tf.x[0] as usize, tf),
        8 => sys_rand(tf.x[0] as u32, tf.x[1] as u32, tf),
        9 => sys_rrand(tf),
        10 => sys_entropy(tf),
        20 => sys_sock_create(tf),
        21 => sys_sock_status(tf.x[0] as usize, tf),
        22 => sys_sock_connect(tf.x[0] as usize, IpAddr::from(tf.x[1], tf.x[2]), tf),
        23 => sys_sock_listen(tf.x[0] as usize, tf.x[1] as u16, tf),
        24 => sys_sock_send(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        25 => sys_sock_recv(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        30 => sys_open(tf.x[0] as usize, tf.x[1] as usize, tf),
        31 => sys_read(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        // 32 => sys_getdent(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        _ => tf.x[7] = OsError::Unknown as u64,
    }
}
