use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::string::String;
use core::time::Duration;
use shim::path::{Path, PathBuf, Component};
use shim::io::{Seek, SeekFrom};
use core::mem::size_of;
use core::ops::Add;
use core::str;
use core::ffi::c_void;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::param::USER_IMG_BASE;
use crate::process::{Process, State, IOHandle};
use crate::traps::TrapFrame;
use crate::{ETHERNET, SCHEDULER, FILESYSTEM};
use crate::vm::{VirtualAddr, Page, PagePerm};

use kernel_api::*;
use kernel_api::fs::Stat;
use pi::timer;
use fat32::traits::{FileSystem, Entry, Dir};
// use kernel_api::cstr::{CString, CStr, CChar};
// use kernel_api::args::CArgs;

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
// pub fn sys_write(b: u8, tf: &mut TrapFrame) {
//     if b.is_ascii() {
//         let ch = b as char;
//         kprint!("{}", ch);
//         tf.x[7] = OsError::Ok as u64;
//     } else {
//         tf.x[7] = OsError::IoErrorInvalidInput as u64;
//     }
// }

pub fn sys_write(handle_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    if len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }
    let result = unsafe { to_user_slice(va, len) }
        .map_err(|_| OsError::BadAddress);

    let buf = match result {
        Ok(buf) => buf,
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };

    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        match process.handles.get_mut(handle_idx) {
            Some(handle) => {
                match handle {
                    IOHandle::Console => {
                        // use core::fmt::Write;
                        //
                        let buf_str = match str::from_utf8(buf) {
                            Ok(str) => str,
                            Err(e) => {
                                tf.x[7] = OsError::from(e) as u64;
                                return
                            }
                        };
                        kprint!("{}", buf_str);
                        tf.x[0] = buf_str.len() as u64;
                        tf.x[7] = OsError::Ok as u64;

                        // match CONSOLE.lock().write_str(buf_str){
                        //     Ok(()) => {
                        //         tf.x[0] = buf_str.len() as u64;
                        //         tf.x[7] = OsError::Ok as u64;
                        //     }
                        //     Err(e) => {
                        //         tf.x[0] = 0;
                        //         tf.x[7] = OsError::from(e) as u64;
                        //     }
                        // }
                    }
                    IOHandle::File(file_handle) => {
                        use shim::io::Write;

                        let bytes = match file_handle.write(&buf) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                tf.x[7] = OsError::from(e) as u64;
                                return
                            }
                        };
                        tf.x[0] = bytes as u64;
                        tf.x[7] = OsError::Ok as u64;
                    }
                    _io_handle => {
                        tf.x[7] = OsError::NotAFile as u64;
                    }
                }
            }
            None => tf.x[7] = OsError::NoEntry as u64,
        }
    });

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

pub fn sys_brk(va: usize, tf: &mut TrapFrame)  {
    SCHEDULER.critical(|scheduler| {
        let next_heap_ptr = VirtualAddr::from(va);
        let mut process = scheduler.find_process(tf);
        if next_heap_ptr.as_u64() >= process.heap_ptr.as_u64() {
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
            tf.x[7] = OsError::Ok as u64;
        } else {
            tf.x[7] = OsError::BadAddress as u64;
        }
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
    let path_result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    let path: &Path = match path_result {
        Ok(path) => path.as_ref(),
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    trace!("sys_open() path: {:?}", path);

    SCHEDULER.critical(|scheduler| {
        let mut process = scheduler.find_process(tf);
        let mut working_path = process.cwd.clone();
        trace!("sys_open() cwd: {:?}", working_path);

        working_path.push(path);
        trace!("sys_open() working path: {:?}", working_path);
        // let working_path = normalize_path(working_path);
        // trace!("sys_open() mormalized path: {:?}", working_path);
        let mut normalized_working_path = PathBuf::new();
        for component in working_path.components() {
            match component {
                Component::ParentDir => {
                    normalized_working_path.pop();
                }
                Component::CurDir => (),
                Component::Prefix(_) => (),
                component => normalized_working_path.push(component),
            }
        }
        trace!("sys_open() normalized_working_path: {:?}", normalized_working_path);


        let entry = match FILESYSTEM.open(normalized_working_path) {
            Ok(entry) => entry,
            Err(e) => {
                tf.x[7] = OsError::from(e) as u64;
                return
            }
        };
        match entry.is_file() {
            true => {
                let file = entry.into_file()
                    .expect("Entry unexpectedly failed to convert to file");
                let file_idx = process.handles.len();
                process.handles.push(IOHandle::File(Box::new(file)));
                tf.x[0] = file_idx as u64;
                tf.x[7] = OsError::Ok as u64;


            }
            false => tf.x[7] = OsError::IoErrorInvalidInput as u64,
        }

    });
}

pub fn sys_read(file_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    use shim::io::Read;

    if len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }

    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        let result = unsafe { to_user_slice_mut(va, len) }
            .map_err(|_| OsError::BadAddress);
        match result {
            Ok(buf) => {
                match process.handles.get_mut(file_idx) {
                    Some(handle) => {
                        match handle {
                            IOHandle::Console => {
                                let byte = CONSOLE.lock().read_byte();
                                buf[0] = byte;
                                tf.x[0] = 1;
                                tf.x[7] = OsError::Ok as u64;
                            }
                            IOHandle::File(file_handle) => {
                                match file_handle.read(buf) {
                                    Ok(bytes) => {
                                        tf.x[0] = bytes as u64;
                                        tf.x[7] = OsError::Ok as u64;
                                    },
                                    Err(e) => {
                                        tf.x[7] = OsError::from(e) as u64;
                                    }
                                };
                            }
                            _io_handle => tf.x[7] = OsError::NotAFile as u64,
                        }
                    }
                    None => tf.x[7] = OsError::NoEntry as u64,
                }
            }
            Err(e) => tf.x[7] = e as u64,
        }
    });
}
pub fn sys_chdir(va: usize, len: usize, tf: &mut TrapFrame) {
    let path_result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    let path: &Path = match path_result {
        Ok(path) => path.as_ref(),
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        let mut working_path = process.cwd.clone();
        trace!("sys_chdir() cwd: {:?}", working_path);
        trace!("sys_chdir() path: {:?}", path);
        working_path.push(path);
        let mut normalized_working_path = PathBuf::new();
        trace!("sys_chdir() working_path: {:?}", working_path);
        for component in working_path.components() {
            match component {
                Component::ParentDir => {
                    normalized_working_path.pop();
                }
                Component::CurDir => (),
                Component::Prefix(_) => (),
                component => normalized_working_path.push(component),
            }
        }

        trace!("sys_chdir() normalized_working_path: {:?}", normalized_working_path);
        // let working_path = normalize_path(working_path);
        match FILESYSTEM.open(&normalized_working_path) {
            Ok(entry) => {
                if entry.is_dir() {
                    process.cwd = normalized_working_path;
                    trace!("sys_chdir() process.cwd: {:?}", &process.cwd);

                    tf.x[7] = OsError::Ok as u64;
                } else {
                    tf.x[7] = OsError::NotADir as u64;
                }
            }
            Err(e) => tf.x[7] = OsError::from(e) as u64,
        };
    });
}

pub fn sys_getcwd(va: usize, len: usize, offset: usize, tf: &mut TrapFrame) {
    let mut buf = match unsafe { to_user_slice_mut(va, len) }
        .map_err(|_| OsError::BadAddress) {
        Ok(buf) => buf,
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };

    SCHEDULER.critical(|scheduler|{
        use shim::io::Read;

        let process = scheduler.find_process(tf);
        let cwd = process.cwd.clone();
        trace!("sys_getcwd() cwd: {:?}", cwd);
        if offset <= cwd.to_str().unwrap().as_bytes().len() {
            match cwd.to_str().unwrap().as_bytes()[offset..].as_ref().read(buf) {
                Ok(bytes) => {
                    tf.x[0] = bytes as u64;
                    tf.x[7] = OsError::Ok as u64;
                }
                Err(e) => tf.x[7] = OsError::from(e) as u64,
            }
        } else {
            tf.x[7] = OsError::IoErrorInvalidData as u64;
        }
    });
}


pub fn sys_getdents(
    path_va: usize,
    path_len: usize,
    buf_va: usize,
    buf_len: usize,
    offset: u64,
    tf: &mut TrapFrame
) {
    trace!("sys_getdents()");
    if buf_len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }

    let overflow = buf_va.checked_add(buf_len * size_of::<fs::DirEnt>()).is_none();
    let buf_result = if buf_va >= USER_IMG_BASE && !overflow {
        Ok(buf_va)
    } else {
        Err(OsError::BadAddress)
    };
    let path_result = unsafe { to_user_slice(path_va, path_len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    let path: &Path = match path_result {
        Ok(path) => path.as_ref(),
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    trace!("sys_getdents() path: {:?}", path);
    match &buf_result {
        Ok(va) => {
            let mut entries = 0u64;
            let working_path = SCHEDULER.critical(|scheduler|{
                let mut process = scheduler.find_process(tf);
                let mut working_path = process.cwd.clone();
                trace!("sys_getdents() working_path: {:?}", working_path);

                working_path.push(path);
                // normalize_path(working_path)
                working_path
            });
            let mut normalized_working_path = PathBuf::new();
            for component in working_path.components() {
                match component {
                    Component::ParentDir => {
                        normalized_working_path.pop();
                    }
                    Component::CurDir => (),
                    Component::Prefix(_) => (),
                    component => normalized_working_path.push(component),
                }
            }

            trace!("sys_getdents() normalized_working_path: {:?}", normalized_working_path);

            let entry = match FILESYSTEM.open(normalized_working_path) {
                Ok(entry) => entry,
                Err(e) => {
                    tf.x[7] = OsError::from(e) as u64;
                    return
                }
            };
            if entry.is_dir() {
                trace!("sys_getdents() is dir");

                let dir = entry.into_dir()
                    .expect("Entry unexpectedly failed to convert to dir");
                trace!("sys_getdents() into_dir() done");
                let mut dir_entries = dir.entries().unwrap();
                trace!("sys_getdents() into_dir() offset: {}", offset);

                if offset != 0 {
                    trace!("sys_getdents() into_dir() setting offset");

                    match dir_entries.seek(SeekFrom::Start(offset)) {
                        Ok(_) => (),
                        Err(e) => {
                            tf.x[7] = OsError::from(e) as u64;
                            return
                        }
                    };
                }

                for index in 0..buf_len {
                    match dir_entries.next() {
                        Some(entry) => {
                            let dent_va = va.add(size_of::<fs::DirEnt>() * index);
                            let mut dent = unsafe { &mut *(dent_va as  *mut fs::DirEnt) };

                            dent.set_name(entry.name());
                            match entry.is_file() {
                                true => dent.set_d_type(fs::DirType::File),
                                false => dent.set_d_type(fs::DirType::Dir),
                            }
                            trace!("sys_getdents() dent: {}", dent);

                            entries += 1;
                        }
                        None => {
                            break
                        }
                    }
                }
                tf.x[0] = entries;
                tf.x[7] = OsError::Ok as u64;
            } else {
                if offset == 0 {
                    trace!("sys_getdents() is not dir");
                    let dent_va = *va;
                    let mut dent = unsafe { &mut *(dent_va as  *mut fs::DirEnt) };
                    dent.set_name(entry.name());
                    dent.set_d_type(fs::DirType::File);
                    trace!("sys_getdents() dent: {}", dent);
                    tf.x[0] = 1;
                    tf.x[7] = OsError::Ok as u64;
                } else {
                    tf.x[0] = 0;
                    tf.x[7] = OsError::Ok as u64;
                }
            }
        }
        Err(e) => tf.x[7] = *e as u64,
    }
}

pub fn sys_stat(path_va: usize, path_len: usize, buf_va: usize, tf: &mut TrapFrame) {
    let overflow = buf_va.checked_add(size_of::<Stat>()).is_none();
    let buf_result = if buf_va >= USER_IMG_BASE && !overflow {
        Ok(buf_va)
    } else {
        Err(OsError::BadAddress)
    };
    let path_result = unsafe { to_user_slice(path_va, path_len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    let path: &Path = match path_result {
        Ok(path) => path.as_ref(),
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    let working_path = SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        let mut working_path = process.cwd.clone();
        working_path.push(path);
        // normalize_path(working_path)
        working_path
    });

    let mut normalized_working_path = PathBuf::new();
    for component in working_path.components() {
        match component {
            Component::ParentDir => {
                normalized_working_path.pop();
            }
            Component::CurDir => (),
            Component::Prefix(_) => (),
            component => normalized_working_path.push(component),
        }
    }


    match &buf_result {
        Ok(va) => {
            let entry = match FILESYSTEM.open(normalized_working_path) {
                Ok(entry) => entry,
                Err(e) => {
                    tf.x[7] = OsError::from(e) as u64;
                    return
                }
            };
            let stat_vs = *va;
            let mut stat = unsafe { &mut *(stat_vs as *mut Stat) };

            stat.set_metadata_from_raw(entry.metadata().raw());
            stat.set_size(entry.size() as u64);

            tf.x[7] = OsError::Ok as u64;
        }
        Err(e) => tf.x[7] = *e as u64,
    }
}

/// Creates a socket and saves the socket handle in the current process's
/// socket list.
///
/// This function does neither take any parameter nor return anything,
/// except the usual return code that indicates successful syscall execution.
pub fn sys_sock_create(tf: &mut TrapFrame) {
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        let sock_idx = process.handles.len();
        let mut socket_handle = ETHERNET.add_socket();
        let io_handle = IOHandle::Socket(socket_handle);
        process.handles.push(io_handle);
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
        match process.handles.get(sock_idx) {
            Some(io_handle) => {
                match io_handle {
                    IOHandle::Socket(handle) => {
                        let (is_active, is_listening, can_send, can_recv) = ETHERNET.with_socket(*handle, |socket| {
                            (socket.is_active(),  socket.is_listening(), socket.can_send(), socket.can_recv())
                        });
                        tf.x[0] = is_active as u64;
                        tf.x[1] = is_listening as u64;
                        tf.x[2] = can_send as u64;
                        tf.x[3] = can_recv as u64;
                        tf.x[7] = OsError::Ok as u64;
                    }
                    _ => tf.x[7] = OsError::InvalidSocket as u64,
                }
            }
            None => tf.x[7] = OsError::InvalidSocket as u64,
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
        match process.handles.get(sock_idx) {
            Some(io_handle) => {
                match io_handle {
                    IOHandle::Socket(handle) => {
                        let port: u16;
                        match ETHERNET.get_ephemeral_port() {
                            Some(p) => port = p,
                            None => {
                                tf.x[7] = OsError::NoEntry as u64;
                                return
                            }
                        };
                        match ETHERNET.mark_port(port) {
                            Some(_) => (),
                            None => {
                                tf.x[7] = OsError::NoEntry as u64;
                                return
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
                    _ => tf.x[7] = OsError::InvalidSocket as u64,
                }
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
        match process.handles.get(sock_idx ) {
            Some(io_handle) => {
                match io_handle {
                    IOHandle::Socket(handle) => {
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
                    _ => tf.x[7] = OsError::InvalidSocket as u64,
                }
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

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized_path = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized_path.pop();
            },
            component=> normalized_path.push(component),
        }
    }
    normalized_path
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
    if len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }
    match unsafe { to_user_slice(va, len) } {
        Ok(data) => {
            SCHEDULER.critical(|scheduler|{
                let mut process = scheduler.find_process(tf);
                match process.handles.get(sock_idx) {
                    Some(io_handle) => {
                        match io_handle {
                            IOHandle::Socket(handle) => {
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
                            _ => tf.x[7] = OsError::InvalidSocket as u64,
                        }
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
    if len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }
    match unsafe { to_user_slice_mut(va, len) } {
        Ok(data) => {
            SCHEDULER.critical(|scheduler|{
                let mut process = scheduler.find_process(tf);
                match process.handles.get(sock_idx) {
                    Some(io_handle) => {
                        match io_handle {
                            IOHandle::Socket(handle) => {
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
                            _ => tf.x[7] = OsError::InvalidSocket as u64,
                        }
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
pub fn sys_write_str(handle_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    if len == 0 {
        tf.x[0] = 0;
        tf.x[7] = OsError::Ok as u64;
        return
    }

    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    match result {
        Ok(str) => {
            SCHEDULER.critical(|scheduler| {
                let mut process = scheduler.find_process(tf);
                match process.handles.get_mut(handle_idx) {
                    Some(handle) => {
                        match handle {
                            IOHandle::Console => {
                                kprint!("{}", str);

                                tf.x[0] = str.len() as u64;
                                tf.x[7] = OsError::Ok as u64;
                            }
                            IOHandle::File(file_handle) => {
                                use shim::io::Write;

                                match file_handle.write(str.as_bytes()) {
                                    Ok(bytes) => {
                                        tf.x[0] = bytes as u64;
                                        tf.x[7] = OsError::Ok as u64;
                                    }
                                    Err(e) => tf.x[7] = OsError::from(e) as u64,
                                };
                            }
                            _ => tf.x[7] = OsError::IoError as u64
                        }
                    }
                    None => tf.x[7] = OsError::NoEntry as u64,
                }
            });

        }
        Err(e) => {
            tf.x[7] = e as u64;
        }
    }
}

fn sys_load_p(va: usize, len: usize, tf: &mut TrapFrame) {
    let path_result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));

    let path: &Path = match path_result {
        Ok(path) => path.as_ref(),
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    // trace!("sys_load_p() path: {:?}", path);
    let cwd = SCHEDULER.critical(|scheduler| {
        let mut current_process = scheduler.find_process(tf);
        let mut working_path = current_process.cwd.clone();
        current_process.cwd.clone()
    });
    // trace!("sys_load_p() cwd: {:?}", working_path);
    let mut working_path = cwd.clone();
    working_path.push(path);
    let mut normalized_working_path = PathBuf::new();
    for component in working_path.components() {
        match component {
            Component::ParentDir => {
                normalized_working_path.pop();
            }
            Component::CurDir => (),
            Component::Prefix(_) => (),
            component => normalized_working_path.push(component),
        }
    }



    // trace!("sys_load_p() working path: {:?}", working_path);
    match Process::load(normalized_working_path, cwd) {
        Ok(process) => {
            // trace!("sys_load_p() process loaded from: {:?}", path);
            match SCHEDULER.add(process) {
                Some(pid) => {
                    // trace!("sys_load_p() process scheduled: {}", pid);
                    tf.x[0] = pid;
                    tf.x[7] = OsError::Ok as u64;
                }
                None => tf.x[7] = OsError::MaxPidExceeded as u64,
            }
        },
        Err(e) => tf.x[7] = e as u64,
    }
}

fn sys_start_p(pid: u64, tf: &mut TrapFrame) {
    trace!("sys_start_p() starting pid: {}", pid);
    // let state = State::Waiting(Box::new(move |p|{
    //     SCHEDULER.critical(|scheduler_internal|{
    //         match scheduler_internal.find_process_by_pid(pid) {
    //             Some(_) => false,
    //             None => true,
    //         }
    //     })
    // }));

    SCHEDULER.critical(|scheduler| {
        match scheduler.find_process_by_pid(pid) {
            Some(process) => {
                process.start();
                tf.x[7] = OsError::Ok as u64;
            },
            None => {
                tf.x[7] = OsError::InvalidPid as u64;
            }
        }

        // match scheduler.find_process_by_pid(pid) {
        //     Some(process) => {
        //         let current_process =  scheduler.find_process(tf);
        //         current_process.state = state;
        //         process.start();
        //         tf.x[7] = OsError::Ok as u64;
        //     }
        //     None => tf.x[7] = OsError::InvalidPid as u64,
        // }
    });
    // let current_process = match current_process_option {
    //     Some(process) => process,
    //     None => {
    //         tf.x[7] = OsError::InvalidPid as u64;
    //         return
    //     }
    // };
    // let mut process = SCHEDULER.critical(|scheduler| -> &mut Process {
    //     scheduler.find_process(tf)
    // });
    // current_process.state = State::Waiting(Box::new(move |p|{
    //     SCHEDULER.critical(|scheduler_internal|{
    //         match scheduler_internal.find_process_by_pid(pid) {
    //             Some(_) => false,
    //             None => true,
    //         }
    //     })
    // }));
    // process.start();
    // tf.x[7] = OsError::Ok as u64;
}

fn sys_wait(pid: u64, tf: &mut TrapFrame) {
    trace!("sys_wait() tf.tpid: {}, pid: {}", tf.tpidr, pid);
    SCHEDULER.switch(State::WaitFor(pid), tf);
    tf.x[7] = OsError::Ok as u64;
}

fn sys_args_count(tf: &mut TrapFrame) {
    SCHEDULER.critical(|scheduler| {
        let mut process = scheduler.find_process(tf);
        tf.x[0] = process.args.len() as u64;
        tf.x[7] = OsError::Ok as u64;
    });
}

fn sys_read_arg(idx: usize, buf_va: usize, buf_len: usize, offset: usize, tf: &mut TrapFrame) {
    let mut buf = match unsafe { to_user_slice_mut(buf_va, buf_len) }
        .map_err(|_| OsError::BadAddress) {
        Ok(buf) => buf,
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    SCHEDULER.critical(|scheduler|{
        let mut process = scheduler.find_process(tf);
        match process.args.get(idx) {
            Some(arg) => {
                use shim::io::Read;
                if offset <= arg.as_bytes().len() {
                    match arg.as_bytes()[offset..].as_ref().read(buf) {
                        Ok(bytes) => {
                            tf.x[0] = bytes as u64;
                            tf.x[7] = OsError::Ok as u64;
                        }
                        Err(e) => tf.x[7] = OsError::from(e) as u64,
                    }
                } else {
                    tf.x[7] = OsError::IoErrorInvalidData as u64;
                }

            }
            None => tf.x[7] = OsError::NoEntry as u64
        }

    });
}

fn sys_push_arg(pid: u64, va: usize, len: usize, tf: &mut TrapFrame) {
    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::Utf8Error));
    let arg = match result {
        Ok(arg) => {
            info!("sys_push_arg() pid: {}, arg: {}", pid, arg);
            arg
        },
        Err(e) => {
            tf.x[7] = e as u64;
            return
        }
    };
    SCHEDULER.critical(|scheduler| {
        match scheduler.find_process_by_pid(pid) {
            Some(proc) => {
                proc.args.push(String::from(arg));
                tf.x[7] = OsError::Ok as u64;
            }
            None => tf.x[7] = OsError::InvalidPid as u64,
        }
    });
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
        4 => sys_write(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        5 => sys_getpid(tf),
        6 => sys_write_str(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        7 => sys_sbrk(tf.x[0] as usize, tf),
        8 => sys_rand(tf.x[0] as u32, tf.x[1] as u32, tf),
        9 => sys_rrand(tf),
        10 => sys_entropy(tf),
        12 => sys_start_p(tf.x[0], tf),
        13 => sys_brk(tf.x[0] as usize, tf),
        14 => sys_args_count(tf),
        15 => sys_read_arg(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf.x[3] as usize, tf),
        16 => sys_push_arg(tf.x[0], tf.x[1] as usize, tf.x[2] as usize, tf),
        17 => sys_load_p(tf.x[0] as usize, tf.x[1] as usize, tf),
        18 => sys_wait(tf.x[0], tf),
        20 => sys_sock_create(tf),
        21 => sys_sock_status(tf.x[0] as usize, tf),
        22 => sys_sock_connect(tf.x[0] as usize, IpAddr::from(tf.x[1], tf.x[2]), tf),
        23 => sys_sock_listen(tf.x[0] as usize, tf.x[1] as u16, tf),
        24 => sys_sock_send(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        25 => sys_sock_recv(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        30 => sys_open(tf.x[0] as usize, tf.x[1] as usize, tf),
        31 => sys_read(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        32 => sys_getdents(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf.x[3] as usize, tf.x[4], tf),
        33 => sys_stat(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        34 => sys_getcwd(tf.x[0] as usize, tf.x[1] as usize, tf.x[2] as usize, tf),
        35 => sys_chdir(tf.x[0] as usize, tf.x[1] as usize, tf),
        _ => tf.x[7] = OsError::Unknown as u64,
    }
}
