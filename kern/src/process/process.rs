use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;
use shim::io;
use shim::path::{Path, PathBuf};
use core::mem;
use core::ops::Add;

use fat32::traits::FileSystem;
use fat32::traits::Entry;
use fat32::vfat::{File, DirIterator, Entry as EntryEnum};

use aarch64;
use smoltcp::socket::SocketHandle;

use crate::param::*;
use crate::process::State;
use crate::traps::TrapFrame;
use crate::vm::*;
use crate::FILESYSTEM;
use crate::fs::PiVFatHandle;
use crate::console::{Console, CONSOLE};

use kernel_api::{OsError, OsResult};

/// Type alias for the type of a process ID.
pub type Id = u64;

/// Type alias for the type of a File Descriptor
pub type Fd = u64;

#[derive(Debug)]
pub enum FdEntry {
    Console,
    File(Box<File<PiVFatHandle>>),
    DirEntries(Box<DirIterator<PiVFatHandle>>),
}

#[derive(Debug)]
pub enum IOHandle {
    StdIn,
    StdOut,
    StdErr,
    File(Box<File<PiVFatHandle>>),
    Socket(SocketHandle),
    Unused,
}

/// A structure that represents the complete state of a process.
#[derive(Debug)]
pub struct Process {
    /// The saved trap frame of a process.
    pub context: Box<TrapFrame>,
    /// The page table describing the Virtual Memory of the process
    pub vmap: Box<UserPageTable>,
    /// The scheduling state of the process.
    pub state: State,
    pub stack_base: VirtualAddr,
    pub heap_ptr: VirtualAddr,
    pub heap_page: VirtualAddr,
    /// Socket handles held by the current process
    pub handles: Vec<IOHandle>,
    pub cwd: PathBuf,
}

impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `Err(OsError)`. Otherwise returns `Ok` of the new `Process`.
    pub fn new() -> OsResult<Process> {
        let vmap = Box::new(UserPageTable::new());
        let sockets: Vec<SocketHandle> = Vec::new();
        Ok(Process {
            context: Box::new(TrapFrame::default()),
            vmap,
            state: State::Ready,
            stack_base: Process::get_stack_base(),
            heap_ptr: VirtualAddr::from(0),
            heap_page: VirtualAddr::from(0),
            handles: vec![IOHandle::StdIn, IOHandle::StdOut, IOHandle::StdErr],
            // last_file_descriptor: Some(1),
            cwd: PathBuf::from("/"),
        })
    }

    /// Loads a program stored in the given path by calling `do_load()` method.
    /// Sets trapframe `context` corresponding to its page table.
    /// `sp` - the address of stack top
    /// `elr` - the address of image base.
    /// `ttbr0` - the base address of kernel page table
    /// `ttbr1` - the base address of user page table
    /// `spsr` - `F`, `A`, `D` bit should be set.
    ///
    /// Returns Os Error if do_load fails.
    pub fn load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        use crate::VMM;

        let mut p = Process::do_load(pn)?;
        p.context.sp = Process::get_stack_top().as_u64();
        p.context.elr = Process::get_image_base().as_u64();
        p.context.ttbr0 = VMM.get_baddr().as_u64();
        p.context.ttbr1 = p.vmap.get_baddr().as_u64();
        p.context.spsr = p.context.spsr |
            aarch64::SPSR_EL1::D |
            aarch64::SPSR_EL1::A |
            aarch64::SPSR_EL1::F;
        Ok(p)
    }

    /// Creates a process and open a file with given path.
    /// Allocates one page for stack with read/write permission, and N pages with read/write/execute
    /// permission to load file's contents.
    fn do_load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        use io::{Write, Read};
        use core::ops::AddAssign;

        let mut p = Process::new()?;
        for page in 0..USER_STACK_PAGE_COUNT {
            let _stack_page = p.vmap.alloc(Process::get_stack_base() + VirtualAddr::from(page * Page::SIZE), PagePerm::RW);
        }
        let pn = pn.as_ref();
        let entry = FILESYSTEM.open(pn)?;

        let mut file = match entry.into_file() {
            Some(file) => file,
            None => return Err(OsError::IoErrorInvalidData)
        };

        let mut current_address = Process::get_image_base();

        loop {
            let buf = p.vmap.alloc(current_address, PagePerm::RWX);
            let bytes = file.read(buf)?;
            current_address.add_assign(VirtualAddr::from(Page::SIZE));
            if bytes == 0 {
                break;
            }
        }
        let _heap_page = p.vmap.alloc(current_address, PagePerm::RW);
        p.heap_ptr = current_address;
        p.heap_page = current_address;
        Ok(p)
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE + (USER_MAX_VM_SIZE - 16))
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE)
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        VirtualAddr::from(USER_STACK_BASE)
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        VirtualAddr::from(USER_STACK_BASE + (USER_STACK_SIZE - 16))
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occurred. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        match self.state {
            State::Ready => true,
            State::Waiting(_) => {
                let mut current_state = mem::replace(&mut self.state, State::Ready);
                let current_ready =  match current_state {
                    State::Waiting(ref mut event_pol_fn) => event_pol_fn(self),
                    State::Ready => true,
                    _ => false,
                };
                if !current_ready {
                    self.state = current_state;
                }
                current_ready
            }
            _ => false,
        }
    }
}
