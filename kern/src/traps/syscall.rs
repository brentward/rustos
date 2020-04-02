use alloc::boxed::Box;
use core::time::Duration;
use core::slice;
use core::str;
use core::ops::BitOr;
use shim::path::PathBuf;

use fat32::traits::FileSystem;

use crate::console::CONSOLE;
use crate::process::State;
use crate::traps::TrapFrame;
use crate::SCHEDULER;
use crate::FILESYSTEM;
use crate::vm::{PageTable, VirtualAddr, PhysicalAddr};
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
        p.context.x[7] = 1;
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

    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        if b.is_ascii() {
            let ch = b as char;
            kprint!("{}", ch);
            p.context.x[7] = 1;
        } else {
            p.context.x[7] = 70;
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
        p.context.x[7] = 1;
        true
    })), tf);
}

pub fn sys_open(path_ptr: u64, path_len: usize, tf: &mut TrapFrame) {
    use crate::console::kprintln;

    SCHEDULER.switch(State::Waiting(Box::new(move |p| {
        match p.last_file_id {
            Some(fid) => {
                let path_pa =  match p.vmap.get_pa(VirtualAddr::from(path_ptr)) {
                    Some(pa) => pa.bitor(PhysicalAddr::from(path_ptr & 0xFFFF)),
                    None => {
                        p.context.x[7] = 104;
                        return true
                    }
                };

                let path_slice =  unsafe {
                    slice::from_raw_parts(path_pa.as_ptr(), path_len)
                };

                let path = match str::from_utf8(path_slice) {
                    Ok(path) => path,
                    Err(_e) => {
                        p.context.x[7] = 50;
                        return true
                    }
                };

                let path_buf = PathBuf::from(path);

                let entry = match FILESYSTEM.open(path_buf.as_path()) {
                    Ok(entry) => entry,
                    Err(_) => {
                        p.context.x[7] = 10;
                        return true
                    }
                };

                p.last_file_id = fid.checked_add(1);
                p.files.push((fid, Box::new(entry)));


                p.context.x[0] = fid;
                p.context.x[7] = 1;

                true

            }
            None => {
                p.context.x[7] = 101;
                return true
            }
        }
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
        6 => sys_open(tf.x[0] as u64, tf.x[1] as usize, tf),
        _ => tf.x[7] = 1,
    }
}
