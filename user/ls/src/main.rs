#![feature(asm)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::{print, println, OsError, OsResult};
use kernel_api::syscall::*;
use bw_allocator::Allocator;
use alloc::string::{String, ToString};
use core::fmt::Write;

#[global_allocator]
pub static A: Allocator = Allocator::new();

fn main() {
    let result = ls();
    if let Err(error) = result {
        let mut s = String::new();
        write!(s, "ls error: {:?}\r\n", error);
        print!("{}", s);
    }

}

fn ls() -> OsResult<()> {
    let screen_width = 100;
    let args = args();
    let mut path = getcwd();
    let mut show_hidden = false;
    let mut human_readable = false;
    let mut long = false;
    let mut result = String::new();
    for (idx, arg) in args.iter().enumerate() {
        if idx == 0 {
            // spin
        } else if arg.starts_with("--") {
            match arg.as_str() {
                "--all" => show_hidden = true,
                "--human-readable" => human_readable = true,
                "--long" => long = true,
                option => {
                    writeln!(result, "ls: invalid option: {}", option)?;
                }
            }
        } else if arg.starts_with("-") {
            for ch in arg.chars() {
                match ch {
                    'a' => show_hidden = true,
                    'h' => human_readable = true,
                    'l' => long = true,
                    '-' => (),
                    option => {
                        writeln!(result, "ls: invalid option: -{}", option)?;
                    }
                }
            }
        } else {
            if args.len() > idx + 1 {
                writeln!(result, "ls: too many ags")?;
            }
            path.push(arg);
            break
        }

    }
    let dents = getdents(path);
    let length = dents.iter()
        .fold(0, |acc, dent| acc.max(dent.to_string().chars().count())) + 2;
    let cols = screen_width / length;
    let mut cur_col = 0;
    for (idx, dent) in dents.iter().enumerate() {
        let stat = stat(dent.name())?;
        if show_hidden || !stat.metadata().hidden() {
            if long {
                writeln!(
                    result,
                    "{}  {:<8}  {}",
                    stat.metadata().to_string(),
                    stat.size_string(human_readable),
                    dent.to_string()
                )?;
            } else {
                write!(
                    result,
                    "{:<width$}",
                    dent.to_string(),
                    width = length
                )?;
                if cur_col == cols || idx + 1 == dents.len() {
                    writeln!(result, "")?;
                    cur_col = 0;
                } else {
                    cur_col += 1;
                }

                // if (result.chars().count() % 120) + length <= 120 {
                //     write!(
                //         result,
                //         "{:<width$}",
                //         dent.to_string(),
                //         width = length
                //     )?;
                // } else {
                //     writeln!(result, "")?;
                //     write!(
                //         result,
                //         "{:<width$}",
                //         dent.to_string(),
                //         width = length
                //     )?;
                // }

            }
        }
    }
    print!("{}", result);
    Ok(())
}