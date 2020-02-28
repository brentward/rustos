use shim::io;
use shim::path::{Path, PathBuf};

use stack_vec::StackVec;

use pi::atags::Atags;
use alloc::string::String;
use alloc::vec;

// use fat32::traits::FileSystem;
// use fat32::traits::{Dir, Entry};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
    Empty,
    TooManyArgs,
}

/// A structure representing a single shell command.
struct Command<'a> {
    args: StackVec<'a, &'a str>,
}

impl<'a> Command<'a> {
    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args = StackVec::new(buf);
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(arg).map_err(|_| Error::TooManyArgs)?;
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// Returns this command's path. This is equivalent to the first argument.
    fn path(&self) -> &str {
        self.args[0]
    }
}

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const BELL: u8 = 7;
const BACK: u8 = 8;
const DEL: u8 = 127;

/// Starts a shell using `prefix` as the prefix for each line. This function
/// never returns.
pub fn shell(prefix: &str) -> ! {
    kprintln!("\r\nBrentWard Shell 0.0.1a");
    loop {
        kprint!("{}", prefix);
        let mut input_buf = [0u8; 512];
        let mut input = StackVec::new(&mut input_buf);
        'read_char: loop {
            let byte = CONSOLE.lock().read_byte();
            match byte {
                DEL | BACK => {
                    if !input.is_empty() {
                        input.pop();
                        CONSOLE.lock().write_byte(BACK);
                        kprint!(" ");
                        CONSOLE.lock().write_byte(BACK);
                    } else {
                        CONSOLE.lock().write_byte(BELL);
                    }
                }
                CR | LF => break 'read_char,
                byte if byte < 32 || byte > 127 => CONSOLE.lock().write_byte(BELL),
                byte => {
                    if input.push(byte).is_ok() {
                        CONSOLE.lock().write_byte(byte);
                    } else {
                        CONSOLE.lock().write_byte(BELL);
                    }
                }
            }
        }
        kprintln!("");
        let input_str = core::str::from_utf8(input.as_slice())
            .expect("input bytes failed to cast back to string");
        let mut args_buf = [""; 64];
        match Command::parse(input_str, &mut args_buf) {
            Ok(command) => {
                match command.path() {
                    "echo" => echo(&command.args),
                    "atags" => atag(&command.args),
                    "panic" => panic!("You called panic"),
                    "unreachable" => unreachable!(),
                    "usemem" => use_memory(),
                    "memstats" => memstats(),
                    path => kprintln!("unknown command: {}", path)
                }
            } // TODO execute command
            Err(Error::TooManyArgs) => {
                kprintln!("error: too many arguments");
            }
            Err(Error::Empty) => (),
        }
    }
}

fn echo(args: &StackVec<&str>) {
    for &arg in args[1..].iter() {
        kprint!("{} ", arg);
    }
    kprint!("\r\n");
}

fn atag(args: &StackVec<&str>) {
    let atags = Atags::get();
    if args.len() > 1 {
        match args[1] {
            "mem" => {
                for atag in atags {
                    match atag {
                        pi::atags::Atag::Mem(_) => kprintln!("{:#?}", atag),
                        _ => (),
                    }
                }
            }
            "core" => {
                for atag in atags.into_iter() {
                    match atag {
                        pi::atags::Atag::Core(_) => kprintln!("{:#?}", atag),
                        _ => (),
                    }
                }
            }
            "cmd" => {
                for atag in atags.into_iter() {
                    match atag {
                        pi::atags::Atag::Cmd(_) => kprintln!("{:#?}", atag),
                        _ => (),
                    }
                }
            }
            "unknown" => {
                for atag in atags.into_iter() {
                    match atag {
                        pi::atags::Atag::Unknown(_) => kprintln!("{:#?}", atag),
                        _ => (),
                    }
                }
            }
            _ => {
                for atag in atags.into_iter() {
                    kprintln!("{:#?}", atag);
                }
            }
        }
    } else {
        for atag in atags.into_iter() {
            kprintln!("{:#?}", atag);
        }
    }
}

fn use_memory() {
    let mut base_string = String::from("hi again");
    let mut string_vec = vec![base_string.clone()];
    for _ in 0..1024 {
        base_string.push_str(", and again");
        let new_string = base_string.clone();
        string_vec.push(new_string);
    };
    kprintln!("{:?}", string_vec[1023]);
}

fn memstats() {
    kprintln!{"{:?}", ALLOCATOR};
}