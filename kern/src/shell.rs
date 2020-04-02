use shim::io;
use shim::io::{Write, Read};
use shim::path::{Path, PathBuf};

use stack_vec::StackVec;

use pi::atags::Atags;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{self, Write as FmtWrite};
use core::time::Duration;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry, Metadata};

use aarch64;

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;
use crate::SCHEDULER;
use crate::process::Process;
use pi::{timer, gpio};

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
    const SEPARATOR: char = ' ';
    const QUOTE: char = '"';

    /// Parse a command from a string `s` using `buf` as storage for the
    /// arguments.
    ///
    /// # Errors
    ///
    /// If `s` contains no arguments, returns `Error::Empty`. If there are more
    /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args = StackVec::new(buf);
        let mut arg_start = 0usize;
        let mut in_quote = false;
        for (index, ch) in s.char_indices() {
            match ch {
                Command::SEPARATOR => {
                    if !in_quote {
                        if arg_start < index {
                            args.push(&s[arg_start..index]
                                .trim_matches('"'))
                                .map_err(|_| Error::TooManyArgs)?;
                        }
                        arg_start = index + 1;
                    }
                },
                Command::QUOTE => {
                    in_quote = !in_quote;
                    if arg_start < index {
                        args.push(&s[arg_start..index]
                            .trim_matches('"'))
                            .map_err(|_| Error::TooManyArgs)?;
                    }
                    arg_start = index + 1;
                }
                _ => (),
            }
        }
        if arg_start < s.len() {
            args.push(&s[arg_start..]
                .trim_matches('"'))
                .map_err(|_| Error::TooManyArgs)?;
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
}

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const BELL: u8 = 7;
const BACK: u8 = 8;
const DEL: u8 = 127;
const MAX_LINE_LEN: usize = 80;

/// Starts a shell using `prefix` as the prefix for each line. This function
/// never returns.
pub fn shell(prefix: &str) {
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
                    "echo" => {
                        match Echo::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "pwd" => {
                        match Pwd::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "cd" => {
                        match  Cd::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "ls" =>{
                        match Ls::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "ll" => {
                        match Ls::new(Some("l")) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    // "ll2" => {
                    //     let mut ll_cmd = Ls::from(Some("l"))?;
                    //     ll_cmd.set_long(true);
                    //     ll_cmd.exec(&command, &mut cwd)
                    // }
                    "cat" => {
                        match Cat::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "exit" => {
                        kprintln!("Goodbye...");
                        break
                    },
                    "sleep" => {
                        match Sleep::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "brk" => {
                        match Brk::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "blink" => {
                        match Blink::new(None) {
                            Ok(ref mut executable) => executable
                                .exec(&command, &mut cwd),
                            Err(e) => Err(e),
                        }
                    },
                    "panic!" => panic!("called panic"),
                    _path => {
                        match BinFile::new(None) {
                            Ok(ref mut executable) => {
                                match executable.exec(&command, &mut cwd) {
                                    Ok(foo) => break,
                                    Err(e) => Err(e),
                                }
                            }
                            Err(e) => Err(e),
                        }
                    },
                };
                match exec_result {
                    Ok(std_out) => {
                        kprint!("{}", std_out.result);
                        error_level = 0;
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
    fn exec(&mut self, _cmd: &Command, _cwd: &mut PathBuf) -> StdResult;
}

struct Echo;

impl Executable for Echo {
    fn new(_params: Option<&str> ) -> ExecutableResult<Echo> {
        Ok(Echo)
    }

    fn exec(&mut self, cmd: &Command, _cwd: &mut PathBuf) -> StdResult {
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

    fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
        let mut result = String::new();

        let mut working_dir = cwd.clone();

        let path = Path::new(cmd.path());

        set_working_dir(&path, &mut working_dir);

        let entry = match FILESYSTEM.open(working_dir.as_path()) {
            Ok(entry) => entry,
            Err(_) => {
                writeln!(result, "bwsh: {}: command not found", cmd.path())?;

                return Err(StdError { result, code: 1 })
            }
        };

        if entry.is_file() {
            let p = match Process::load(working_dir.as_path()) {
                Ok(process) => process,
                Err(e) => {
                    writeln!(result, "bwsh: error running command: {:#?}", e)?;

                    return Err(StdError { result, code: 1 })
                }
            };
            SCHEDULER.add(p);

        } else {
            writeln!(result, "bwsh: {}: is a directory", cmd.path())?;

            return Err(StdError { result, code: 1 })
        }

        Ok(StdOut { result })
    }
}

struct Pwd;

impl Executable for Pwd {
    fn new(_params: Option<&str>) -> ExecutableResult<Pwd> {
        Ok(Pwd)
    }

    fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
        let mut result = String::new();
        if cmd.args.len() != 1 {
            writeln!(result, "pwd: too many arguments")?;

            Err(StdError { result, code: 1 })
        } else {
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

    fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
        let mut result = String::new();
        let mut path = Path::new("/");
        let mut working_dir = cwd.clone();
        if cmd.args.len() > 2 {
            writeln!(result, "cd: too many arguments")?;

            return Err(StdError { result, code: 1 });
        } else if cmd.args.len() == 2 {
            path = Path::new(cmd.args[1]);
        }

        set_working_dir(&path, &mut working_dir);

        let entry = match FILESYSTEM.open(working_dir.as_path()) {
            Ok(entry) => entry,
            Err(_) => {
                writeln!(result, "cd: no such file or directory: {}", path.to_str().unwrap())?;

                return Err(StdError { result, code: 1 });
            }
        };

        match entry.as_dir() {
            Some(_) => {
                while cwd.pop() {
                    cwd.pop();
                }
                cwd.push(working_dir.as_path());

                Ok(StdOut { result })
            }
            None => {
                writeln!(result, "cd: not a directory: {}", path.to_str().unwrap())?;

                Err(StdError { result, code: 1 })
            }
        }
    }
}

struct Ls {
    show_hidden: bool,
    human_readable: bool,
    long: bool,
}

impl Executable for Ls {
    fn new(params: Option<&str>) -> ExecutableResult<Ls> {
        match params {
            Some(params) => {
                let mut show_hidden = false;
                let mut human_readable = false;
                let mut long = false;
                for ch in params.chars() {
                    match ch {
                        'a' => show_hidden = true,
                        'h' => human_readable = true,
                        'l' => long = true,
                        param => {
                            let mut result = String::new();
                            writeln!(result, "ls: invalid param: {}", param)?;

                            return Err(StdError {
                                result,
                                code: 1,
                            })
                        },
                    }
                }
                Ok(Ls {
                    show_hidden,
                    human_readable,
                    long,
                })
            }
            None => Ok(Ls {
                show_hidden: false,
                human_readable: false,
                long: false,
            }),
        }
    }

    fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
        let mut result = String::new();
        let mut option_end = cmd.args.len();
        if cmd.args.len() > 1 {
            for arg_index in 1..cmd.args.len() {
                if cmd.args[arg_index].len() > 2 && &cmd.args[arg_index][..2] == "--" {
                    match &cmd.args[arg_index][2..] {
                        "all" => self.show_hidden = true,
                        "human-readable" => self.human_readable = true,
                        "long" => self.long = true,
                        option => {
                            writeln!(result, "ls: invalid option: --{}", option)?;

                            return Err(StdError { result, code: 1 });
                        }
                    }
                } else if cmd.args[arg_index].len() > 1 && &cmd.args[arg_index][..1] == "-" {
                    for arg in cmd.args[arg_index][1..].chars() {
                        match arg {
                            'a' => self.show_hidden = true,
                            'h' => self.human_readable = true,
                            'l' => self.long = true,
                            option => {
                                writeln!(result, "ls: invalid option: -{}", option)?;

                                return Err(StdError { result, code: 1 });
                            }
                        }
                    }
                } else {
                    option_end = arg_index;
                    break
                }
            }
        }
        if cmd.args.len() > option_end + 1 {
            writeln!(result, "ls: too many arguments")?;

            return Err(StdError { result, code: 1 });
        }
        let path = if cmd.args.len() > option_end {
            Path::new(cmd.args[option_end])
        } else {
            Path::new(".")
        };

        let mut working_dir = cwd.clone();

        set_working_dir(&path, &mut working_dir);

        let entry = match FILESYSTEM.open(working_dir.as_path()) {
            Ok(entry) => entry,
            Err(_) => {
                writeln!(result, "ls: no such file or directory: {}", path.to_str()
                    .expect("path is not valid unicode"))?;

                return Err(StdError { result, code: 1 });
            }
        };
        match entry.as_dir() {
            Some(dir) => {
                let entries = dir.entries().unwrap().collect::<Vec<_>>();
                let length = entries.iter()
                    .fold(0, |acc, entry| acc.max(entry.display_name().len())) + 2;
                for entry in entries {
                    if self.show_hidden || !entry.metadata().hidden() {
                        if self.long {
                            let mut size = String::new();
                            if self.human_readable {
                                entry.write_human_size(&mut size)?;
                            } else {
                                entry.write_size(&mut size)?;
                            }
                            writeln!(result, "{}  {:<8}  {}",
                                   entry.metadata().to_string(),
                                   size,
                                   entry.display_name(),)?;

                        } else {
                            if (result.len() % MAX_LINE_LEN) + length <= MAX_LINE_LEN {
                                write!(
                                    result,
                                    "{:<width$}",
                                    entry.display_name(),
                                    width = length
                                )?;
                            } else {
                                writeln!(result, "")?;
                                write!(
                                    result,
                                    "{:<width$}",
                                    entry.display_name(),
                                    width = length
                                )?;
                            }
                        }
                    }
                }
                if !self.long {
                    writeln!(result, "")?;
                }
            }
            None => {
                if self.show_hidden || !entry.metadata().hidden() {
                    if self.long {
                        let mut size = String::new();
                        if self.human_readable {
                            entry.write_human_size(&mut size)?;
                        } else {
                            entry.write_size(&mut size)?;
                        }
                        writeln!(result, "{}  {:<8}  {}",
                                 entry.metadata().to_string(),
                                 size,
                                 entry.display_name(),)?;

                    } else {
                        writeln!(result, "{}", entry.display_name(),)?
                    }
                }
            }
        }

        Ok(StdOut { result })
    }
}

struct Cat;

impl Executable for Cat {
    fn new(_params: Option<&str>) -> ExecutableResult<Cat> {
        Ok(Cat)
    }

    fn exec(&mut self, cmd: &Command, cwd: &mut PathBuf) -> StdResult {
        let mut result = String::new();
        for &arg in cmd.args[1..].iter() {
            let mut working_dir = cwd.clone();

            let path = Path::new(&arg);

            set_working_dir(&path, &mut working_dir);

            let entry = match FILESYSTEM.open(working_dir.as_path()) {
                Ok(entry) => entry,
                Err(_) => {
                    writeln!(&mut result, "cat: {} no such fhe or directory", path.to_str()
                        .expect("path is not valid unicode"))?;

                    return Err(StdError { result, code: 1 });
                }
            };

            let mut file_vec = Vec::new();
            let mut bytes_read = 0usize;
            let total_size = entry.size();
            let mut file = match entry.into_file() {
                Some(file) => file,
                None => {
                    writeln!(result, "cat: {}: is a directory", path.to_str()
                        .expect("path is not valid unicode"))?;

                    return Err(StdError { result, code: 1 });
                }
            };
            while bytes_read < total_size {
                let mut buf = [0u8;1024];
                let bytes = match file.read(&mut buf) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        writeln!(result, "cat: {}: file could not be opened", path.to_str()
                            .expect("path is not valid unicode"))?;

                        return Err(StdError { result, code: 1 });
                    }
                };
                let _bytes_written = file_vec.write(&buf)
                    .expect("failed to write to vector");
                bytes_read += bytes;
            }
            while file_vec.len() > bytes_read {
                file_vec.pop();
            }
            match String::from_utf8(file_vec) {
                Ok(string) => {
                    writeln!(result, "{}", string.as_str())?;
                },
                Err(_) => {
                    writeln!(result, "cat: {}: file not valid UTF-8", path.to_str()
                        .expect("path is not valid unicode"))?;

                    return Err(StdError { result, code: 1 });
                }
            }
        }

        Ok(StdOut { result })
    }
}


struct Brk;

impl Executable for Brk {
    fn new(_params: Option<&str>) -> ExecutableResult<Brk> {
        Ok(Brk)
    }

    fn exec(&mut self, _cmd: &Command, _cwd: &mut PathBuf) -> StdResult {
        let result = String::new();
        aarch64::brk!(2);

        Ok(StdOut { result })
    }
}

struct Blink;

impl Executable for Blink {
    fn new(_params: Option<&str>) -> ExecutableResult<Blink> {
        Ok(Blink)
    }

    fn exec(&mut self, _cmd: &Command, _cwd: &mut PathBuf) -> StdResult {
        let mut process_1 = Process::new().expect("Process::new() failed");
        process_1.context.elr = run_blinky as u64;
        process_1.context.sp = process_1.stack.top().as_u64();
        process_1.context.spsr = 0b1_10100_0000;
        SCHEDULER.add(process_1);
        let result = String::new();
        Ok(StdOut { result })

    }
}

struct Sleep;

impl Executable for Sleep {
    fn new(_params: Option<&str>) -> ExecutableResult<Sleep> {
        Ok(Sleep)
    }

    fn exec(&mut self, _cmd: &Command, _cwd: &mut PathBuf) -> StdResult {
        let result = String::new();
        kernel_api::syscall::sleep(Duration::from_secs(10));

        Ok(StdOut { result })
    }
}

fn set_working_dir(path: &Path, cwd: &mut PathBuf) {
    if path.is_absolute() {
        while cwd.pop() { }
    }

    for dir in path.iter() {
        if dir.to_str().unwrap() == "." {
        } else if dir.to_str().unwrap() == ".." {
            cwd.pop();
        } else {
            cwd.push(Path::new(dir))
        }
    }
}

pub extern "C" fn run_blinky() {
    let mut gpio16 = gpio::Gpio::new(16).into_output();
    loop {
        gpio16.set();
        timer::spin_sleep(Duration::from_secs(2));
        gpio16.clear();
        timer::spin_sleep(Duration::from_secs(2));
    }
}
