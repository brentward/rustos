use alloc::boxed::Box;
use shim::io;
use shim::path::Path;
use core::mem;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry, Metadata};

use aarch64;

use crate::param::*;
use crate::process::{Stack, State};
use crate::traps::TrapFrame;
use crate::vm::*;
use crate::FILESYSTEM;

use kernel_api::{OsError, OsResult};

/// Type alias for the type of a process ID.
pub type Id = u64;

/// A structure that represents the complete state of a process.
#[derive(Debug)]
pub struct Process {
    /// The saved trap frame of a process.
    pub context: Box<TrapFrame>,
    /// The memory allocation used for the process's stack.
    pub stack: Stack,
    /// The page table describing the Virtual Memory of the process
    pub vmap: Box<UserPageTable>,
    /// The scheduling state of the process.
    pub state: State,
}

impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `Err(OsError)`. Otherwise returns `Ok` of the new `Process`.
    pub fn new() -> OsResult<Process> {
        let stack = match Stack::new() {
            Some(stack) => stack,
            None => return Err(OsError::NoMemory),
        };
        let vmap = Box::new(UserPageTable::new());
        Ok(Process {
            context: Box::new(TrapFrame::default()),
            stack,
            vmap,
            state: State::Ready,
        })
    }

    /// Load a program stored in the given path by calling `do_load()` method.
    /// Set trapframe `context` corresponding to the its page table.
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
        p.context.tpidr = p.vmap.get_baddr().as_u64();
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
        let stack_page = p.vmap.alloc(Process::get_stack_base(), PagePerm::RW);
        let pn = pn.as_ref();
        let entry = FILESYSTEM.open(pn)?;

        // let mut file_vec = Vec::new();
        // let mut bytes_read = 0usize;
        // let total_size = entry.size();
        let mut file = match entry.into_file() {
            Some(file) => file,
            None => return Err(OsError::IoErrorInvalidData)
        };
        let mut current_address = Process::get_image_base();
        loop {
            let mut buf = p.vmap.alloc(current_address, PagePerm::RWX);
            let bytes = file.read(buf)?;
            if bytes == 0 {
                break;
            }
            current_address.add_assign(VirtualAddr::from(Page::SIZE));
            // let _bytes_written = file_vec.write(&buf)
            //     .expect("failed to write to vector");
            // bytes_read += bytes;
        }
         Ok(p)
        // while file_vec.len() > bytes_read {
        //     file_vec.pop();
        // }
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE + USER_MAX_VM_SIZE)
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE)
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        VirtualAddr::from(USER_STACK_BASE - Page::SIZE)
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        VirtualAddr::from(USER_STACK_BASE - Stack::ALIGN)
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
    ///     occured. If it has, the state is switched to `Ready` and this
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
                    _ => panic!("unexpected match in current_state"),
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
