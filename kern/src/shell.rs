use shim::io;
use shim::io::{Write, Read};
use shim::path::{Path, PathBuf};

use stack_vec::StackVec;

use pi::atags::Atags;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry, Metadata};

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
    let mut cwd = PathBuf::from("/");
    let mut error_level = 0u8;
    kprintln!("\r\nBrentward Shell (bwsh: 0.0.1a)");
    loop {
        kprint!("{} {}", cwd.as_path().to_str().expect("cwd path is not valid Unicode"), prefix);
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
                let exec_result = match command.path() {
                    "echo" => Echo::exec(&command, &mut cwd),
                    "pwd" => Pwd::exec(&command, &mut cwd),
                    "cd" => Cd::exec(&command, &mut cwd),
                    "ls" => Ls::exec(&command, &mut cwd),
                    "cat" => Cat::exec(&command, &mut cwd),
                    // "pwd" => pwd(),
                    // "cd" => cd(&command.args),
                    // "ls" => ls(&command.args),
                    // "cat" => cat(&command.args),
                    // "atags" => atag(&command.args),
                    // "panic" => panic!("You called panic"),
                    // "unreachable" => unreachable!(),
                    // "usemem" => use_memory(),
                    // "memstats" => memstats(),
                    _path => Unknown::exec(&command, &mut cwd),
                };
                match exec_result {
                    Ok(std_out) => {
                        kprint!("{}", std_out.result);
                        error_level = std_out.code;
                    }
                    Err(std_err) => {
                        kprint!("{}", std_err.result);
                        error_level = std_err.code;
                    }
                }
            } // TODO execute command
            Err(Error::TooManyArgs) => {
                kprintln!("bwsh: too many arguments");
            }
            Err(Error::Empty) => (),
        }
    }
}

pub struct StdOut {
    pub result: String,
    pub code: u8
}

struct StdErr{
    pub result: String,
    pub code: u8
}


trait Executable {
    fn exec(cmd: &Command, _cwd: &mut PathBuf) -> Result<StdOut, StdErr>;
}

struct Echo;

impl Executable for Echo {
    fn exec(cmd: &Command, _cwd: &mut PathBuf) ->Result<StdOut, StdErr> {
        let mut result = String::new();
        for &arg in cmd.args[1..].iter() {
            result.push_str(arg);
            result.push(' ');
        }
        if result.len() > 0 {
            result.pop();
        }

        Ok(StdOut { result, code: 0 })
    }
}

struct Unknown;

impl Executable for Unknown {
    fn exec(cmd: &Command, _cwd: &mut PathBuf) -> Result<StdOut, StdErr> {
        let mut result = String::from("bwsh: command not found: ");
        result.push_str(cmd.path());
        result.push_str("\r\n");

        Err(StdErr { result, code: 1 })
    }
}

struct Pwd;

impl Executable for Pwd {
    fn exec(cmd: &Command, cwd: &mut PathBuf) ->Result<StdOut, StdErr> {
        if cmd.args.len() != 1 {
            let result = String::from("pwd: too many arguments\r\n");
            Err(StdErr { result, code: 1 })
        } else {
            let path = cwd.as_path().to_str().expect("path is not valid unicode");
            let mut result = String::from(path);
            result.push_str("\r\n");
            // let result = String::from(cwd.to_str());
            Ok(StdOut { result, code: 0 })
        }
    }
}

struct Cd;

impl Executable for Cd {
    fn exec(cmd: &Command, cwd: &mut PathBuf) ->Result<StdOut, StdErr> {
        let mut path = Path::new("/");
        let mut working_dir = cwd.clone();
        if cmd.args.len() > 2 {
            let result = String::from("cd: too many arguments\r\n");
            return Err(StdErr { result, code: 1 });
        } else if cmd.args.len() == 2 {
            path = Path::new(cmd.args[1]);
        // } else {
        //     while cwd.pop() {
        //         cwd.pop();
        //     }
        //     return Ok(StdOut { result: String::new(), code: 0})
        }
        if path.is_absolute() {
            while working_dir.pop() {
                working_dir.pop();
            }
        }

        for dir in path.iter() {
            if dir.to_str().unwrap() == "." {
            } else if dir.to_str().unwrap() == ".." {
                working_dir.pop();
            } else {
                working_dir.push(Path::new(dir))
            }
        }

        // working_dir.push(path);

        let entry = match FILESYSTEM.open(working_dir.as_path()) {
            Ok(entry) => entry,
            Err(_) => {
                let mut result = String::from("cd: no such file or directory: ");
                result.push_str( path.to_str()
                    .expect("path is not valid unicode"));
                result.push_str("\r\n");
                return Err(StdErr { result, code: 1 });
            }
        };

        match entry.as_dir() {
            Some(_) => {
                while cwd.pop() {
                    cwd.pop();
                }

                cwd.push(working_dir.as_path());
                Ok(StdOut { result: String::new(), code: 0 })
                //exists, set dir
            }
            None => {
                let mut result = String::from("cd: not a directory: ");
                result.push_str( path.to_str()
                    .expect("path is not valid unicode"));
                result.push_str("\r\n");
                Err(StdErr { result, code: 1 })
            }
        }
    }
}

struct Ls;

impl Executable for Ls {
    fn exec(cmd: &Command, cwd: &mut PathBuf) ->Result<StdOut, StdErr> {
        let mut option_end = cmd.args.len();
        let mut show_hidden = false;
        if cmd.args.len() > 1 {
            for arg_index in 1..cmd.args.len() {
                if &cmd.args[arg_index][..1] != "-" {
                    option_end = arg_index;
                    break
                }
            }
        }
        for param in cmd.args[1..option_end].iter() {
            match param {
                &"-a" => show_hidden = true,
                &option => {
                    let mut result = String::from("ls: invalid option: ");
                    result.push_str(option);
                    result.push_str("\r\n");
                    return Err(StdErr { result, code: 1 });
                }
            }
        }
        if cmd.args.len() > option_end + 1 {
            let result = String::from("ls: too many arguments\r\n");
            return Err(StdErr { result, code: 1 });
        }
        let path = if cmd.args.len() > option_end {
            Path::new(cmd.args[option_end])
        } else {
            Path::new(".")
        };

        let mut working_dir = cwd.clone();

        if path.is_absolute() {
            while working_dir.pop() {
                working_dir.pop();
            }
        }

        for dir in path.iter() {
            if dir.to_str().unwrap() == "." {
            } else if dir.to_str().unwrap() == ".." {
                working_dir.pop();
            } else {
                working_dir.push(Path::new(dir))
            }
        }

        let entry = match FILESYSTEM.open(working_dir.as_path()) {
            Ok(entry) => entry,
            Err(_) => {
                let mut result = String::from("cd: no such file or directory: ");
                result.push_str( path.to_str()
                    .expect("path is not valid unicode"));
                result.push_str("\r\n");
                return Err(StdErr { result, code: 1 });
            }
        };
        let mut result = String::new();
        match entry.as_dir() {
            Some(dir) => {
                let entries = dir.entries().unwrap().collect::<Vec<_>>();
                for entry in entries {
                    if show_hidden || !entry.metadata().hidden() {
                        let name = if entry.metadata().directory() {
                            let mut name = String::from(entry.name());
                            name.push('/');
                            name
                        } else {
                            String::from(entry.name())
                        };
                        let size = get_size(entry.size());
                        let mut size_spacer = String::new();
                        for _ in 0..(8 - size.len()) {
                            size_spacer.push(' ');
                        }
                        result.push_str(entry.metadata().to_string().as_str());
                        result.push_str("   ");
                        result.push_str(size.as_str());
                        result.push_str(size_spacer.as_str());
                        result.push_str("   ");
                        result.push_str(name.as_str());
                        result.push_str("\r\n");
                    }
                }
            }
            None => {
                if show_hidden || !entry.metadata().hidden() {
                    let name = if entry.metadata().directory() {
                        let mut name = String::from(entry.name());
                        name.push('/');
                        name
                    } else {
                        String::from(entry.name())
                    };
                    let size = get_size(entry.size());
                    let mut size_spacer = String::new();
                    for _ in 0..(8 - size.len()) {
                        size_spacer.push(' ');
                    }
                    result.push_str(entry.metadata().to_string().as_str());
                    result.push_str("   ");
                    result.push_str(size.as_str());
                    result.push_str(size_spacer.as_str());
                    result.push_str("   ");
                    result.push_str(name.as_str());
                    result.push_str("\r\n");
                }
            }
        }

        Ok(StdOut { result, code: 0 })
    }
}

struct Cat;

impl Executable for Cat {
    fn exec(cmd: &Command, cwd: &mut PathBuf) ->Result<StdOut, StdErr> {
        let mut result = String::new();
        for &arg in cmd.args[1..].iter() {
            let mut working_dir = cwd.clone();

            let path = Path::new(&arg);
            for dir in path.iter() {
                if dir.to_str().unwrap() == "." {
                } else if dir.to_str().unwrap() == ".." {
                    working_dir.pop();
                } else {
                    working_dir.push(Path::new(dir))
                }
            }

            if path.is_absolute() {
                while working_dir.pop() {
                    working_dir.pop();
                }
            }

            let entry = match FILESYSTEM.open(working_dir.as_path()) {
                Ok(entry) => entry,
                Err(_) => {
                    let mut result = String::from("cat: ");
                    result.push_str( path.to_str()
                        .expect("path is not valid unicode"));
                    result.push_str(": no such file or directory");
                    result.push_str("\r\n");
                    return Err(StdErr { result, code: 1 });
                }
            };

            let mut file_vec = Vec::new();
            let mut bytes_read = 0usize;
            let total_size = entry.size();
            let mut file = match entry.into_file() {
                Some(file) => file,
                None => {
                    let mut result = String::from("cat: ");
                    result.push_str(path.to_str()
                        .expect("path is not valid unicode"));
                    result.push_str(": is a directory");
                    result.push_str("\r\n");
                    return Err(StdErr { result, code: 1 });
                }
            };
            while bytes_read < total_size {
                let mut buf = [0u8;1024];
                let bytes = match file.read(&mut buf) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        let mut result = String::from("cat: ");
                        result.push_str( path.to_str()
                            .expect("path is not valid unicode"));
                        result.push_str(": file could not be opened");
                        result.push_str("\r\n");
                        return Err(StdErr { result, code: 1 });
                    }
                };
                let bytes_written = file_vec.write(&buf)
                    .expect("failed to write to vector");
                bytes_read += bytes;
            }
            while file_vec.len() > bytes_read {
                file_vec.pop();
            }
            match String::from_utf8(file_vec) {
                Ok(string) => {
                    result.push_str(string.as_str());
                    result.push_str("\r\n");
                },
                Err(_) => {
                    let mut result = String::from("cat: ");
                    result.push_str( path.to_str()
                        .expect("path is not valid unicode"));
                    result.push_str(": file not valid UTF-8");
                    result.push_str("\r\n");
                    return Err(StdErr { result, code: 1 });
                }
            }
        }

        Ok(StdOut { result, code: 0 })
    }
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

fn get_size(size: usize) -> String {
    match size {
        size@ 0..=1023 => {
            let mut size_str = size.to_string();
            size_str.push_str(" B");
            size_str
        },
        size@ 1024..=1_048_575 => {
            let mut size_str = (size / 1024).to_string();
            size_str.push_str(" KiB");
            size_str
        },
        size@ 1_048_576..=1_073_741_823 => {
            let mut size_str = (size / 1_048_576).to_string();
            size_str.push_str(" MiB");
            size_str
        },
        size => {
            let mut size_str = (size / 1_073_741_824).to_string();
            size_str.push_str(" GiB");
            size_str
        },
    }
}