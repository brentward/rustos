use alloc::boxed::Box;
use core::time::Duration;
use core::slice;
use core::str;
use core::ffi::c_void;
use core::mem::size_of;

use core::ops::{Add, AddAssign, BitAnd, BitOr, Sub, SubAssign};

use shim::path::PathBuf;

use fat32::traits::{FileSystem, Entry, Dir};
use fat32::vfat::Entry as EntryEnum;

use crate::console::{kprint, CONSOLE};
use crate::param::USER_IMG_BASE;
use crate::process::{State, FdEntry};
use crate::traps::TrapFrame;
use crate::SCHEDULER;
use crate::FILESYSTEM;
use crate::vm::{PageTable, VirtualAddr, PhysicalAddr, PagePerm, Page};
use kernel_api::*;
use pi::timer;

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
            p.context.x[7] = 1;
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
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        p.context.x[0] = seconds;
        p.context.x[1] = nanoseconds;
        true
    })), tf);
}

/// Kills current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    let _pid_option = SCHEDULER.kill(tf);
}

/// Write to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    use crate::console::kprint;

    SCHEDULER.switch(State::Waiting(Box::new(move |_p| {
        if b.is_ascii() {
            let ch = b as char;
            kprint!("{}", ch);
        }
        true
    })), tf);
}

/// Returns current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    let pid = tf.tpidr;
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        p.context.x[0] = pid;
        true
    })), tf);
}

pub fn sys_open(path_ptr: usize, path_len: usize, tf: &mut TrapFrame) {
    use crate::console::kprintln;

    let result = unsafe { to_user_slice(path_ptr, path_len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));

    match result {
        Ok(path) => {
            let path_buf = PathBuf::from(path);

            let entry = match FILESYSTEM.open(path_buf.as_path()) {
                Ok(entry) => entry,
                Err(_) => {
                    tf.x[7] = OsError::NoEntry as u64;
                }
            };
            if p.unused_file_descriptors.len() > 0 {
                let fd = p.unused_file_descriptors.pop()
                    .expect("Unexpected p.unused_file_descriptors.pop() failed after len check");
                match  p.file_table[fd] {
                    Some(_) => {
                        tf.x[7] = OsError::IoErrorInvalidData as u64;
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
                        tf.x[0] = fd as u64;
                        tf.x[7] = OsError::Ok as u64;
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
                tf.x[0] = fd as u64;
                tf.x[7] = OsError::Ok as u64;
                true
            }

        }
        Err(e) => {
            p.context.x[7] = e as u64;
            true
        }


    //     SCHEDULER.switch(State::Waiting(Box::new(move |p| {
    //     // let path_slice = unsafe { match p.vmap
    //     //     .get_slice_at_va(VirtualAddr::from(path_ptr), path_len) {
    //     //     Ok(slice) => slice,
    //     //     Err(_) => {
    //     //         p.context.x[7] = 104;
    //     //         return true
    //     //     }
    //     // }};
    //     //
    //     // let path = match str::from_utf8(path_slice) {
    //     //     Ok(path) => path,
    //     //     Err(_e) => {
    //     //         p.context.x[7] = 50;
    //     //         return true
    //     //     }
    //     // };
    //     let result = unsafe { to_user_slice(path_ptr, path_len) }
    //         .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));
    //
    //     match result {
    //         Ok(path) => {
    //             let path_buf = PathBuf::from(path);
    //
    //             let entry = match FILESYSTEM.open(path_buf.as_path()) {
    //                 Ok(entry) => entry,
    //                 Err(_) => {
    //                     p.context.x[7] = OsError::NoEntry as u64;
    //                     return true
    //                 }
    //             };
    //             if p.unused_file_descriptors.len() > 0 {
    //                 let fd = p.unused_file_descriptors.pop()
    //                     .expect("Unexpected p.unused_file_descriptors.pop() failed after len check");
    //                 match  p.file_table[fd] {
    //                     Some(_) => {
    //                         p.context.x[7] = OsError::IoErrorInvalidData as u64;
    //                         true
    //                     }
    //                     None => {
    //                         match entry.is_file() {
    //                             true => {
    //                                 let file = entry.into_file()
    //                                     .expect("Entry unexpectedly failed to convert to file");
    //                                 p.file_table[fd] = Some(FdEntry::File(Box::new(file)));
    //                             }
    //                             false => {
    //                                 let dir = entry.into_dir()
    //                                     .expect("Entry unexpectedly failed to convert to dir");
    //                                 let dir_entries = dir.entries().unwrap(); //FIXME
    //                                 p.file_table[fd] = Some(FdEntry::DirEntries(Box::new(dir_entries)));
    //                             }
    //                         }
    //                         p.context.x[0] = fd as u64;
    //                         p.context.x[7] = OsError::Ok as u64;
    //                         true
    //                     }
    //                 }
    //
    //             } else {
    //                 let fd = p.file_table.len();
    //                 match entry.is_file() {
    //                     true => {
    //                         let file = entry.into_file()
    //                             .expect("Entry unexpectedly failed to convert to file");
    //                         p.file_table.push(Some(FdEntry::File(Box::new(file))));
    //                     }
    //                     false => {
    //                         let dir = entry.into_dir()
    //                             .expect("Entry unexpectedly failed to convert to dir");
    //                         let dir_entries = dir.entries().unwrap(); //FIXME
    //                         p.file_table.push(Some(FdEntry::DirEntries(Box::new(dir_entries))));
    //                     }
    //                 }
    //                 p.context.x[0] = fd as u64;
    //                 p.context.x[7] = OsError::Ok as u64;
    //                 true
    //             }
    //
    //         }
    //         Err(e) => {
    //             p.context.x[7] = e as u64;
    //             true
    //         }
    //     }
    // })), tf);
}

pub fn sys_read(fd: usize, buf_ptr: u64, len: usize, tf: &mut TrapFrame) {
    use crate::console::kprintln;
    use shim::io::Read;

    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        let mut buf_slice = unsafe { match p.vmap
            .get_mut_slice_at_va(VirtualAddr::from(buf_ptr), len) {
            Ok(slice) => slice,
            Err(_) => {
                p.context.x[7] = 104;
                return true
            }
        }};

        match p.file_table.remove(fd) {
            Some(entry) => {
                match entry {
                    FdEntry::Console => {
                        let byte =  CONSOLE.lock().read_byte();
                        p.file_table.insert(fd, Some(FdEntry::Console));
                        buf_slice[0] = byte;
                        p.context.x[0] = 1;
                        p.context.x[7] = 1;
                        true

                    }
                    FdEntry::File(mut file) => {
                        let bytes = match file.read(&mut buf_slice) {
                            Ok(bytes) => bytes,
                            Err(_) => {
                                p.context.x[7] = 101;
                                return true
                            }
                        };
                        p.file_table.insert(fd, Some(FdEntry::File(file)));
                        p.context.x[0] = bytes as u64;
                        p.context.x[7] = 1;
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
    })), tf);
}

pub fn sys_sbrk(size: u64, tf: &mut TrapFrame)  {
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        let next_heap_ptr = p.heap_ptr.add(VirtualAddr::from(size));
        while p.next_heap_page.as_u64() < next_heap_ptr.as_u64() {
            let next_heap_page = p.next_heap_page.add(VirtualAddr::from(Page::SIZE));
            if next_heap_page.as_u64() >= p.stack_base.as_u64() {
                p.context.x[7] = 30;
                return true
            }
            let _heap_page = p.vmap.alloc(p.next_heap_page, PagePerm::RW);
            p.next_heap_page = next_heap_page;
        }
        p.context.x[0] = p.heap_ptr.as_u64();
        p.context.x[7] = 1;
        p.heap_ptr = next_heap_ptr;
        true
    })), tf);
}

pub fn sys_getdent(fd: usize, dent_buf_ptr: u64, count: usize, tf: &mut TrapFrame) {
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        let mut entries = 0u64;
        let mut dir_entries = match p.file_table.remove(fd) {
            Some(entry) => {
                match entry {
                    FdEntry::Console => {
                        p.file_table.insert(fd, Some(FdEntry::Console));
                        p.context.x[7] = 0;
                        return true
                    }
                    FdEntry::File(file) => {
                        p.file_table.insert(fd, Some(FdEntry::File(file)));
                        p.context.x[7] = 0;
                        return true
                    }
                    FdEntry::DirEntries(dir_entries) => dir_entries
                }
            }
            None => {
                p.context.x[7] = 10;
                return true
            }
        };
        for index in 0..count {
            match dir_entries.next() {
                Some(entry) => {

                    let mut dent_buf_pa = match p.vmap.get_pa(VirtualAddr::from(dent_buf_ptr)) {
                        Some(pa) => pa,
                        None => {
                            p.context.x[7] = 0;
                            return true
                        },
                    };
                    let dent_pa = dent_buf_pa.add(PhysicalAddr::from(size_of::<fs::DirEnt>() * index));
                    let mut dent = unsafe { &mut *(dent_pa.as_usize() as  *mut fs::DirEnt) };

                    dent.set_name(entry.name());
                    match entry.is_file() {
                        true => dent.set_d_type(fs::DirType::File),
                        false => dent.set_d_type(fs::DirType::Dir),
                    }
                    entries += 1;

                }
                None => {
                    break
                }
            }
        }
        p.file_table.insert(fd, Some(FdEntry::DirEntries(dir_entries)));
        p.context.x[0] = entries;
        p.context.x[7] = 1;

        true
    })), tf);
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

pub fn sys_unknown(tf: &mut TrapFrame)  {
    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        p.context.x[7] = 0;
        true
    })), tf);
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    use crate::console::kprintln;

    match num {
        1 => sys_sleep(tf.x[0] as u32, tf),
        2 => sys_time(tf),
        3 => sys_exit(tf),
        4 => sys_write(tf.x[0] as u8, tf),
        5 => sys_getpid(tf),
        6 => sys_write_str(tf.x[0] as usize, tf.x[1] as usize, tf),
        7 => sys_sbrk(tf.x[0] as u64, tf),
        8 => sys_open(tf.x[0] as usize, tf.x[1] as usize, tf),
        9 => sys_read(tf.x[0] as usize, tf.x[1] as u64, tf.x[2] as usize, tf),
        10 => sys_getdent(tf.x[0] as usize, tf.x[1] as u64, tf.x[2] as usize, tf),

        _ => sys_unknown(tf),
    }
}
