#![feature(asm)]
#![feature(panic_info_message)]
#![feature(never_type)]
#![no_std]
#![no_main]

extern crate alloc;

mod cr0;

use kernel_api::{print, println, OsError, OsResult};
use kernel_api::fs::Handle;
use kernel_api::syscall::*;
use bw_allocator::Allocator;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::{self, Write};
use shim::path::PathBuf;

use crate::cr0::{STD_IN, STD_OUT};
use kernel_api::fs::Handle::StdIn;

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const BELL: u8 = 7;
const BACK: u8 = 8;
const DEL: u8 = 127;


#[global_allocator]
pub static A: Allocator = Allocator::new();

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
    Empty,
}

struct Command<'a> {
    args: Vec<&'a str>,
}

impl<'a> Command<'a> {
    const SEPARATOR: char = ' ';
    const QUOTE: char = '"';

    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str) -> Result<Command<'a>, Error> {
        let mut args = Vec::new();
        let mut arg_start = 0;
        let mut in_quote = false;
        for (index, ch) in s.char_indices() {
            match ch {
                Command::SEPARATOR => {
                    if !in_quote {
                        if arg_start < index {
                            args.push(s[arg_start..index]
                                .trim_matches('"'));
                        }
                        arg_start = index + 1;
                    }
                },
                Command::QUOTE => {
                    in_quote = !in_quote;
                    if arg_start < index {
                        args.push(s[arg_start..index]
                            .trim_matches('"'));
                    }
                    arg_start = index + 1;
                }
                _ => (),
            }
        }
        if arg_start < s.len() {
            args.push(s[arg_start..]
                .trim_matches('"'));
        }
        // for arg in s.split(' ').filter(|a| !a.is_empty()) {
        //     args.push(arg).map_err(|_| Error::TooManyArgs)?;
        // }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        self.args[0]
    }

    fn args(&self) -> Vec<String> {
        let mut args = Vec::new();
        for arg in &self.args {
            args.push(String::from(arg.clone()));
        }
        args
    }
}

fn main() {
    loop {
        let result = bwsh();
        if let Err(error) = result {
            let mut s = String::new();
            writeln!(s, "Bwsh error: {:?}", error);
            print!("{}", s);
        }
    }
}

fn bwsh() -> OsResult<!> {
    // let mut args = Vec::new();
    // for idx in 0..25 {
    //     let mut arg = String::new();
    //     write!(arg, "Argument # {}!", idx)?;
    //     println!("pushing: {}", arg);
    //     args.push(arg);
    // }
    // println!("calling execve with: {:?}", args);
    // execve(args)?;
    // let mut arg = String::new();
    // write!(arg, "Argument # {}!", 0)?;
    // println!("calling execve() with: {}", arg);
    // execve(arg)?;
    let mut prefix = String::from(" > ");
    let mut error_level = 0u8;
    println!("\nBrentward Shell (bwsh: 0.0.2a)");

    loop {
        let mut cwd = getcwd();

        // let mut cwd_bytes = 0;
        // let mut cwd_v = Vec::<u8>::new();
        // loop {
        //     use shim::io::Write;
        //
        //     let mut cwd_buf = [0u8; 512];
        //     let bytes = getcwd(&mut cwd_buf, cwd_bytes)?;
        //     if bytes == 0 {
        //         break
        //     }
        //     cwd_bytes += bytes;
        //     let _bytes = cwd_v.write(&cwd_buf[..bytes]);
        // }
        // let cwd = String::from_utf8(cwd_v).unwrap();
        let mut out_str = String::new();
        write!(out_str, " {}{}", cwd.to_str().unwrap(), prefix);

        write(&STD_OUT, out_str.as_bytes())?;
        const BUF_LEN: usize = 512;
        let mut cmd_buf = Vec::new();
        'read_char: loop {
            let mut buf = [0u8];
            let _b = read(&STD_IN, &mut buf)?;
            match buf[0] {
                DEL | BACK => {
                    if cmd_buf.len() > 0 {
                        cmd_buf.pop();
                        write(&STD_OUT, &[BACK, b' ', BACK])?;
                    } else {
                        write(&STD_OUT, &[BELL])?;
                    }
                }
                CR | LF => break 'read_char,
                byte if byte < 32 || byte > 127 => {
                    write(&STD_OUT, &[BELL])?;
                },
                byte => {
                    cmd_buf.push(byte);
                    write(&STD_OUT, &[byte])?;
                }

            }
        }
        let s = core::str::from_utf8(cmd_buf.as_slice())
            .expect("input bytes failed to cast back to string");
        match Command::parse(s) {
            Ok(command) => {
                println!("");
                let exec_result = match command.path() {
                    "echo" => {
                        match Echo::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command),
                            Err(e) => Err(e),
                        }
                    },
                    "pwd" => {
                        match Pwd::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command),
                            Err(e) => Err(e),
                        }
                    },
                    "cd" => {
                        match  Cd::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command),
                            Err(e) => Err(e),
                        }
                    },
                    "exit" => {
                        println!("Goodbye...");
                        exit();
                        Ok(StdOut { result: String::new() })
                    },
                    "dice" => {
                        match Dice::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command),
                            Err(e) => Err(e),
                        }
                    },
                    _path => {
                        match BinFile::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command),
                            Err(e) => Err(e),
                        }
                    },
                };
                match exec_result {
                    Ok(std_out) => {
                        print!("{}", std_out.result);
                        error_level = 0;
                    }
                    Err(std_err) => {
                        print!("{}", std_err.result);
                        error_level = std_err.code;
                    }
                }
            } // TODO execute command
            Err(Error::Empty) => println!(""),
            Err(e) => println!("bwsh: error: {:?}", e)
        }

        // let path = match args.get(0) {
        //     Some(path) => {
        //         print!("\n");
        //         path
        //     },
        //     None => continue,
        // };
        // println!("Bwsh: arg len: {}", args.len());
        // for arg in &args {
        //     println!("arg: {}", arg);
        // }
        // write(&STD_OUT, &[LF])?;
        // let pid = execve(path, &args)?;
        // wait(pid)?;
    }
}


pub type StdResult = Result<StdOut, StdError>;

pub type ExecutableResult<T> = Result<T, StdError>;

pub struct StdOut {
    pub result: String,
}

pub struct StdError {
    pub result: String,
    pub code: u8
}

impl From<fmt::Error> for StdError {
    fn from(_error: fmt::Error) -> Self {
        StdError {
            result: String::from("Format error"),
            code: 1,
        }
    }
}

trait Executable: core::marker::Sized {
    fn new(params: Option<&str>) -> ExecutableResult<Self>;
    fn exec(&mut self, _cmd: &Command) -> StdResult;
}

struct Echo;

impl Executable for Echo {
    fn new(_params: Option<&str> ) -> ExecutableResult<Echo> {
        Ok(Echo)
    }

    fn exec(&mut self, cmd: &Command) -> StdResult {
        let mut result = String::new();
        for &arg in cmd.args[1..].iter() {
            write!(result, "{} ", arg)?;
        }
        if result.len() > 0 {
            result.pop();
        }
        writeln!(result, "")?;

        Ok(StdOut { result })
    }
}

struct BinFile;

impl Executable for BinFile {
    fn new(_params: Option<&str>) -> ExecutableResult<BinFile> {
        Ok(BinFile)
    }

    fn exec(&mut self, cmd: &Command) -> StdResult {
        let mut result = String::new();
        // let mut args: Vec::new();
        // for arg in cmd.args {
        //     args.push(String::from(arg));
        // }
        // let args: Vec<String> = cmd.args.iter().map(|arg|String::from(*arg)).collect();
        match execve(cmd.path(), &cmd.args()) {
            Ok(pid) => {
                match wait(pid) {
                    Ok(_) => Ok(StdOut { result }),
                    Err(e) => {
                        writeln!(result, "bwsh: error calling wait(): {:?}", e);
                        Err(StdError { result, code: 1 })
                    }
                }
            }
            Err(e) => {
                writeln!(result, "bwsh: error starting {}: {:?}", cmd.path(), e);
                Err(StdError { result, code: 1 })
            }
        }
        // let mut result = String::new();
        // write!(result, "bwsh: command not found: {}", cmd.path())?;
        //
        // Err(StdError { result, code: 1 })
    }
}

// struct BinFile;
//
// impl Executable for BinFile {
//     fn new(_params: Option<&str>) -> ExecutableResult<BinFile> {
//         Ok(BinFile)
//     }
//
//     fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
//         let mut result = String::new();
//
//         let mut working_dir = cwd.clone();
//
//         let path = Path::new(cmd.path());
//
//         set_working_dir(&path, &mut working_dir);
//
//         let entry = match FILESYSTEM.open(working_dir.as_path()) {
//             Ok(entry) => entry,
//             Err(_) => {
//                 write!(result, "bwsh: {}: command not found", cmd.path())?;
//
//                 return Err(StdError { result, code: 1 })
//             }
//         };
//
//         if entry.is_file() {
//             let p = match Process::load(working_dir.as_path()) {
//                 Ok(process) => process,
//                 Err(e) => {
//                     write!(result, "bwsh: error running command: {:#?}", e)?;
//
//                     return Err(StdError { result, code: 1 })
//                 }
//             };
//             SCHEDULER.add(p);
//
//         } else {
//             write!(result, "bwsh: {}: is a directory", cmd.path())?;
//
//             return Err(StdError { result, code: 1 })
//         }
//
//         Ok(StdOut { result })
//     }
// }
//
struct Pwd;

impl Executable for Pwd {
    fn new(_params: Option<&str>) -> ExecutableResult<Pwd> {
        Ok(Pwd)
    }

    fn exec(&mut self, cmd: &Command) -> StdResult {
        let mut result = String::new();
        if cmd.args.len() != 1 {
            writeln!(result, "pwd: too many arguments")?;

            Err(StdError { result, code: 1 })
        } else {
            let cwd = getcwd();
            writeln!(result, "{}", cwd.as_path().to_str().expect("path is not valid unicode"))?;

            Ok(StdOut { result })
        }
    }
}

struct Cd;

impl Executable for Cd {
    fn new(_params: Option<&str>) -> ExecutableResult<Cd> {
        Ok(Cd)
    }

    fn exec(&mut self, cmd: &Command) -> StdResult {
        let mut result = String::new();
        if cmd.args.len() > 2 {
            writeln!(result, "cd: too many arguments")?;

            Err(StdError { result, code: 1 })
        } else if cmd.args.len() == 2 {
            let path = PathBuf::from(cmd.args[1]);
            match chdir(path) {
                Ok(_) => Ok(StdOut { result }),
                Err(_) => {
                    writeln!(result, "cd: no such file or directory: {}", cmd.args[1])?;
                    Err(StdError { result, code: 1 })
                }
            }
        } else {
            let path = PathBuf::from("/");
            match chdir(path) {
                Ok(_) => Ok(StdOut { result }),
                Err(_) => {
                    writeln!(result, "cd: no such file or directory: /")?;
                    Err(StdError { result, code: 1 })
                }
            }

        }
    }
}

struct Dice {
    verbose: bool,
    count: usize,
}

impl Executable for Dice {
    fn new(_params: Option<&str>) -> ExecutableResult<Self> {
        Ok(Dice {
            verbose: true,
            count: 1,
        })
    }

    fn exec(&mut self, cmd: &Command) -> StdResult {
        let mut result = String::new();
        if cmd.args.len() > 1 {
            for arg_index in 1..cmd.args.len() {
                if cmd.args[arg_index].len() > 2 && &cmd.args[arg_index][..2] == "--" {
                    match &cmd.args[arg_index][2..] {
                        "quiet" => self.verbose = false,
                        "verbose" => self.verbose = true,
                        option => {
                            writeln!(result, "dice: invalid option: --{}", option)?;

                            return Err(StdError { result, code: 1 });
                        }
                    }
                } else if cmd.args[arg_index].len() > 1 && &cmd.args[arg_index][..1] == "-" {
                    for arg in cmd.args[arg_index][1..].chars() {
                        match arg {
                            'q' => self.verbose = false,
                            'v' => self.verbose = true,
                            option => {
                                writeln!(result, "dice: invalid option: -{}", option)?;

                                return Err(StdError { result, code: 1 });
                            }
                        }
                    }
                } else {
                    match cmd.args[arg_index].parse::<usize>() {
                        Ok(count) => self.count = count,
                        Err(e) => {
                            writeln!(result, "dice: invalid argument, count should be a positive integer: {:?}", e)?;

                            return Err(StdError { result, code: 1 })
                        }
                    }
                }
            }
        }
        for die in 0..self.count {
            let rand_num = rand(1, 7);
            if self.verbose {
                writeln!(result, "Die #{}: {}", die + 1, rand_num)?;
            } else {
                writeln!(result, "{}", rand_num)?;
            }
        }
        Ok(StdOut { result })
    }
}