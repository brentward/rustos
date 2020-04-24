use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;

use core::ffi::c_void;
use core::fmt;
use core::mem;
use core::time::Duration;

use aarch64::*;
use pi::local_interrupt::{LocalInterrupt, LocalController, local_tick_in};
use smoltcp::time::Instant;
use pi::{interrupt, timer};

use crate::mutex::Mutex;
use crate::net::uspi::TKernelTimerHandle;
use crate::param::*;
use crate::percore::{get_preemptive_counter, is_mmu_ready, local_irq};
use crate::process::{Id, Process, State};
use crate::traps::irq::IrqHandlerRegistry;
use crate::traps::{TrapFrame, irq};
use crate::{VMM, GLOABAL_IRQ, SCHEDULER, ETHERNET, USB};

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Box<Scheduler>>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enters a critical region and execute the provided closure with a mutable
    /// reference to the inner scheduler.
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

    /// Loops until it finds the next process to schedule.
    /// Call `wfi()` in the loop when no process is ready.
    /// For more details, see the documentation on `Scheduler::switch_to()`.
    ///
    /// Returns the process's ID when a ready process is found.
    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                trace!(
                    "[core-{}] switch_to {:?}, pc: {:x}, lr: {:x}, x29: {:x}, x28: {:x}, x27: {:x}",
                    affinity(),
                    id,
                    tf.elr,
                    tf.x[30],
                    tf.x[29],
                    tf.x[28],
                    tf.x[27]
                );
                return id;
            // } else {
            //     unsafe { asm!("brk 1" :::: "volatile"); }
            }

            aarch64::wfi();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal
    /// conditions.
    pub fn start(&self) -> ! {
        // info!("SCHEDULER::start() for core-{}/@sp={:016x}", affinity(), SP.get());
        let core = affinity();
        if core == 0 {
            self.initialize_global_timer_interrupt();
        }

        // if core != 1 {
        //     pi::timer::spin_sleep(Duration::from_secs(5));
        // }

        // info!("SCHEDULER before local int init for core-{}/@sp={:016x}", affinity(), SP.get());

        self.initialize_local_timer_interrupt();
        // info!("SCHEDULER before adding trap frame for core-{}/@sp={:016x}", affinity(), SP.get());

        let mut tf = TrapFrame::default();
        // info!("SCHEDULER before switch_to using 0 trapframe core-{}/@sp={:016x}", affinity(), SP.get());

        self.switch_to(&mut tf);
        // info!("SCHEDULER after switch_to core-{}/@sp={:016x}", affinity(), SP.get());

        info!("before unsafe core-{}/@sp={:016x}", core, SP.get());

        // let core_stack = KERN_STACK_BASE - (KERN_STACK_SIZE * core);

        unsafe {
            asm!(
                "mov SP, $0 // move tf of the first ready process into SP
                 bl context_restore // restore tf as into running context
                 mrs x0, MPIDR_EL1
                 and x0, x1, #0xff
                 msub x0, x0, $2, $1 // multiply the core number by the kernel stack size and add to kernel stack base and store in x0
                 mov SP, x0 // move the calculated stack for the core address into SP
                 mov x0, xzr // zero out all registers used
                 eret"
                 :: "r"(&tf as *const TrapFrame), "r"(KERN_STACK_BASE), "r"(KERN_STACK_SIZE)
                 : "x0"
                 : "volatile"
            );
        }
        // unsafe {
        //     asm!(
        //         "mov
        //          mov SP, $0 // move tf of the first ready process into SP
        //          bl context_restore // restore tf as into running context
        //          mrs x0, MPIDR_EL1
        //          and x0, x1, #0xff
        //          msub x0, x0, $2, $1 // multiply the core number by the kernel stack size and add to kernel stack base and store in x0
        //          mov SP, x0 // move the calculated stack for the core address into SP
        //          mov x0, xzr // zero out all registers used
        //          eret"
        //          :: "r"(&tf as *const TrapFrame), "r"(core_stack)
        //          : "x0"
        //          : "volatile"
        //     );
        // }

        loop {}
    }

    /// # Lab 4
    /// Initializes the global timer interrupt with `pi::timer`. The timer
    /// should be configured in a way that `Timer1` interrupt fires every
    /// `TICK` duration, which is defined in `param.rs`.
    ///
    /// # Lab 5
    /// Registers a timer handler with `Usb::start_kernel_timer` which will
    /// invoke `poll_ethernet` after 1 second.
    pub fn initialize_global_timer_interrupt(&self) {
        // let mut controller = interrupt::Controller::new();
        // controller.enable(interrupt::Interrupt::Timer1);
        // timer::tick_in(TICK);
        // GLOABAL_IRQ.register(interrupt::Interrupt::Timer1, Box::new(|tf|{
        //     timer::tick_in(TICK);
        //     SCHEDULER.switch(State::Ready, tf);
        // }));
    }

    /// Initializes the per-core local timer interrupt with `pi::local_interrupt`.
    /// The timer should be configured in a way that `CntpnsIrq` interrupt fires
    /// every `TICK` duration, which is defined in `param.rs`.
    pub fn initialize_local_timer_interrupt(&self) {
        // // let mut local_controller = LocalController::new(affinity());
        // // local_controller.enable_local_timer();
        // // local_controller.tick_in(TICK);
        // local_tick_in(affinity(), TICK);
        // // let local_irq = local_irq();
        // local_irq().register(LocalInterrupt::CntpnsIrq, Box::new(|tf|{
        //     let core = affinity();
        //     // let mut local_controller = LocalController::new(affinity());
        //     // local_controller.tick_in(TICK);
        //     local_tick_in(core, TICK);
        //     SCHEDULER.switch(State::Ready, tf);
        // }));
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler.
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Box::new(Scheduler::new()));
        // let proc_count: usize = 32;
        // for _proc in 0..proc_count {
        //     let process = match Process::load("/fib") {
        //         Ok(process) => process,
        //         Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        //     };
        //     self.add(process);
        // }
        //
        let process_0 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        };
        let process_1 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_1::load(): {:#?}", e),
        };
        let process_2 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_2::load(): {:#?}", e),
        };
        let process_3 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_3::load(): {:#?}", e),
        };
        let process_4 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        };
        let process_5 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_1::load(): {:#?}", e),
        };
        let process_6 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_2::load(): {:#?}", e),
        };
        let process_7 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_3::load(): {:#?}", e),
        };
        let process_8 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        };
        let process_9 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_1::load(): {:#?}", e),
        };
        let process_10 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_2::load(): {:#?}", e),
        };
        let process_11 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_3::load(): {:#?}", e),
        };
        let process_12 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_0::load(): {:#?}", e),
        };
        let process_13 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_1::load(): {:#?}", e),
        };
        let process_14 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_2::load(): {:#?}", e),
        };
        let process_15 = match Process::load("/fib") {
            Ok(process) => process,
            Err(e) => panic!("GlobalScheduler::initialize() process_3::load(): {:#?}", e),
        };

        self.add(process_0);
        self.add(process_1);
        self.add(process_2);
        self.add(process_3);
        self.add(process_4);
        self.add(process_5);
        self.add(process_6);
        self.add(process_7);
        self.add(process_8);
        self.add(process_9);
        self.add(process_10);
        self.add(process_11);
        self.add(process_12);
        self.add(process_13);
        self.add(process_14);
        self.add(process_15);
        // let mut controller = interrupt::Controller::new();
        // controller.enable(interrupt::Interrupt::Timer1);
        // timer::tick_in(TICK);
        // GLOABAL_IRQ.register(interrupt::Interrupt::Timer1, Box::new(|tf|{
        //     timer::tick_in(TICK);
        //     SCHEDULER.switch(State::Ready, tf);
        // }));
    }

    // The following method may be useful for testing Lab 4 Phase 3:
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

/// Poll the ethernet driver and re-register a timer handler using
/// `Usb::start_kernel_timer`.
extern "C" fn poll_ethernet(_: TKernelTimerHandle, _: *mut c_void, _: *mut c_void) {
    // Lab 5 2.B
    unimplemented!("poll_ethernet")
}

/// Internal scheduler struct which is not thread-safe.
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
    /// as `Dead` state. Releases all process resources held by the process,
    /// removes the dead process from the queue, drops the dead process's
    /// instance, and returns the dead process's process ID.
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

    /// Releases all process resources held by the current process such as sockets.
    fn release_process_resources(&mut self, tf: &mut TrapFrame) {
        // Lab 5 2.C
        unimplemented!("release_process_resources")
    }

    /// Finds a process corresponding with tpidr saved in a trap frame.
    /// Panics if the search fails.
    pub fn find_process(&mut self, tf: &TrapFrame) -> &mut Process {
        for i in 0..self.processes.len() {
            if self.processes[i].context.tpidr == tf.tpidr {
                return &mut self.processes[i];
            }
        }
        panic!("Invalid TrapFrame");
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.processes.len();
        write!(f, "  [Scheduler] {} processes in the queue\n", len)?;
        for i in 0..len {
            write!(
                f,
                "    queue[{}]: proc({:3})-{:?} \n",
                i, self.processes[i].context.tpidr, self.processes[i].state
            )?;
        }
        Ok(())
    }
}

pub extern "C" fn  test_user_process() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!("mov x0, $2
              svc 1
              mov $0, x0
              mov $1, x7"
                 : "=r"(elapsed_ms), "=r"(error)
                 : "r"(ms)
                 : "x0", "x7"
                 : "volatile");
        }
    }
}
