use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::fmt;

use aarch64::*;
use pi::{interrupt, timer};

use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::{TrapFrame, irq};
use crate::{VMM, IRQ, SCHEDULER};

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enter a critical region and execute the provided closure with the
    /// internal scheduler.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }


    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(move |scheduler| scheduler.add(process))
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfe();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentaion on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&self) -> ! {
        unsafe {
            asm!(
                "mov SP, $0 // move tf of the first process into SP
                 bl context_restore
                 adr x0, _start // store _start address in x0
                 mov SP, x0 // move _start address into SP
                 mov x0, xzr // zero out the register used
                 eret"
                 :: "r"(&*self.0.lock().as_mut().unwrap().processes[0].context)
                 :: "volatile"
            );
        }
        loop {}
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Scheduler::new());

        let process_0 = match Process::load("/fib_new.bin") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        };
        let process_1 = match Process::load("/fib_loop.bin") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_1::load(): {:#?}", e),
        };
        let process_2 = match Process::load("/fib_loop.bin") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_2::load(): {:#?}", e),
        };
        let process_3 = match Process::load("/fib_loop.bin") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_3::load(): {:#?}", e),
        };

        self.add(process_0);
        self.add(process_1);
        self.add(process_2);
        self.add(process_3);

        let mut controller = interrupt::Controller::new();
        controller.enable(interrupt::Interrupt::Timer1);
        timer::tick_in(TICK);
        IRQ.register(interrupt::Interrupt::Timer1, Box::new(|tf|{
            timer::tick_in(TICK);
            SCHEDULER.switch(State::Ready, tf);
        }));
    }

    // The following method may be useful for testing Phase 3:
    //
    // * A method to load a extern function to the user process's page table.
    //
    // pub fn test_phase_3(&self, proc: &mut Process){
    //     use crate::vm::{VirtualAddr, PagePerm};
    //
    //     let mut page = proc.vmap.alloc(
    //         VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);
    //
    //     let text = unsafe {
    //         core::slice::from_raw_parts(test_user_process as *const u8, 24)
    //     };
    //
    //     page[0..24].copy_from_slice(text);
    // }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            last_id: Some(0),
        }
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        match self.last_id {
            Some(id) => {
                self.last_id = id.checked_add(1);
                process.context.tpidr = id;
                self.processes.push_back(process);
                Some(id)
            }
            None => None
        }
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        if self.processes.len() == 0 {
            false
        } else {
            let running_process_id = tf.tpidr;
            let mut running_process_index = self.processes.len();
            for (index, process) in self.processes.iter().enumerate() {
                if process.context.tpidr == running_process_id {
                    running_process_index = index;
                    break;
                }
            };
            if running_process_index == self.processes.len() {
                false
            } else {
                let mut running_process = self.processes.remove(running_process_index)
                    .expect("Unexpected invalid index in Schedule.processes");
                running_process.state = new_state;
                running_process.context = Box::new(*tf);
                self.processes.push_back(running_process);
                true
            }
        }
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        let mut next_process_index = self.processes.len();
        for (index, process) in self.processes.iter_mut().enumerate() {
            if process.is_ready() {
                next_process_index = index;
                break
            }
        }
        if next_process_index == self.processes.len() {
            None
        } else {
            let mut next_process = self.processes.remove(next_process_index)
                .expect("Unexpected invalid index in Schedule.processes");
            next_process.state = State::Running;

            *tf = *next_process.context;
            self.processes.push_front(next_process);
            Some(tf.tpidr)
        }
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        if self.schedule_out(State::Dead, tf) {
            let dead_process = self.processes.pop_back()
                .expect("Unexpected empty Schedule.process");
            let dead_process_id = dead_process.context.tpidr.clone();
            drop(dead_process_id);
            Some(dead_process_id)
        } else {
            None
        }
    }
}
