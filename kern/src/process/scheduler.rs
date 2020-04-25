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
use crate::rng::RNG;

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
        let core = affinity();
        if core == 0 {
            self.initialize_global_timer_interrupt();
        }

        self.initialize_local_timer_interrupt();
        let mut tf = TrapFrame::default();
        self.switch_to(&mut tf);
        info!("SCHEDULER::start() on core-{}/@sp={:016x}", affinity(), SP.get());
        // let rand = {
        //     let mut rng = RNG.lock();
        //     rng.rand(0, 100)
        // };
        // info!("core-{} with rand: {}", affinity(), rand);
        // pi::timer::spin_sleep(Duration::from_millis(core as u64 * 42));
        let xs = Box::new([0u64; 31]);
        info!("box before: {:?}", xs);
        unsafe {
            asm!(
                "mov SP, $31 // move tf of the first ready process into SP
                 bl context_restore // restore tf as into running context
                 mov $0, x0
                 mov $1, x1
                 mov $2, x2
                 mov $3, x3
                 mov $4, x4
                 mov $5, x5
                 mov $6, x6
                 mov $7, x7
                 mov $8, x8
                 mov $9, x9
                 mov $10, x10
                 mov $11, x11
                 mov $12, x12
                 mov $13, x13
                 mov $14, x14
                 mov $15, x15
                 mov $16, x16
                 mov $17, x17
                 mov $18, x18
                 mov $19, x19
                 mov $20, x20
                 mov $21, x21
                 mov $22, x22
                 mov $23, x23
                 mov $24, x24
                 mov $25, x25
                 mov $26, x26
                 mov $27, x27
                 mov $28, x28
                 mov $29, x29
                 mov $30, x30"
                 : "=m"(xs[0]), "=m"(xs[1]), "=m"(xs[2]), "=m"(xs[3]), "=m"(xs[4]), "=m"(xs[5]),
                 "=m"(xs[6]), "=m"(xs[7]), "=m"(xs[8]), "=m"(xs[9]), "=m"(xs[10]), "=m"(xs[11]),
                 "=m"(xs[12]), "=m"(xs[13]), "=m"(xs[14]), "=m"(xs[15]), "=m"(xs[16]), "=m"(xs[17]),
                 "=m"(xs[18]), "=m"(xs[19]), "=m"(xs[20]), "=m"(xs[21]), "=m"(xs[22]), "=m"(xs[23]),
                 "=m"(xs[24]), "=m"(xs[25]), "=m"(xs[26]), "=m"(xs[27]), "=m"(xs[28]), "=m"(xs[29]),
                 "=m"(xs[30])
                 : "r"(&tf as *const TrapFrame)
                 :: "volatile"
            );
            info!("box middle: {:?}", xs);
            asm!(
                "mrs x0, MPIDR_EL1
                 and x0, x0, #0xff
                 msub x0, x0, $32, $31
                 mov SP, x0 // move the calculated stack for the core address into SP
                mov x0, $0
                mov x1, $1
                mov x2, $2
                mov x3, $3
                mov x4, $4
                mov x5, $5
                mov x6, $6
                mov x7, $7
                mov x8, $8
                mov x9, $9
                mov x10, $10
                mov x11, $11
                mov x12, $12
                mov x13, $13
                mov x14, $14
                mov x15, $15
                mov x16, $16
                mov x17, $17
                mov x18, $18
                mov x19, $19
                mov x20, $20
                mov x21, $21
                mov x22, $22
                mov x23, $23
                mov x24, $24
                mov x25, $25
                mov x26, $26
                mov x27, $27
                mov x28, $28
                mov x29, $29
                mov x30, $30
                 eret"
                 :: "m"(xs[0]), "m"(xs[1]), "m"(xs[2]), "m"(xs[3]), "m"(xs[4]), "m"(xs[5]),
                "m"(xs[6]), "m"(xs[7]), "m"(xs[8]), "m"(xs[9]), "m"(xs[10]), "m"(xs[11]),
                "m"(xs[12]), "m"(xs[13]), "m"(xs[14]), "m"(xs[15]), "m"(xs[16]), "m"(xs[17]),
                "m"(xs[18]), "m"(xs[19]), "m"(xs[20]), "m"(xs[21]), "m"(xs[22]), "m"(xs[23]),
                "m"(xs[24]), "m"(xs[25]), "m"(xs[26]), "m"(xs[27]), "m"(xs[28]), "m"(xs[29]),
                "m"(xs[30]), "r"(KERN_STACK_BASE), "r"(KERN_STACK_SIZE)
                 : "volatile"
            );
        }

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
        local_tick_in(affinity(), TICK);
        local_irq().register(LocalInterrupt::CntpnsIrq, Box::new(|tf|{
            let core = affinity();
            local_tick_in(core, TICK);
            SCHEDULER.switch(State::Ready, tf);
        }));
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler.
    pub unsafe fn initialize(&self) {
        *self.0.lock() = Some(Box::new(Scheduler::new()));
        let proc_count: usize = 32;
        for proc in 0..proc_count {
            let process = match Process::load("/fib_rand") {
                Ok(process) => process,
                Err(e) => panic!("GlobalScheduler::initialize() process_{}::load(): {:#?}", proc, e),
            };
            self.add(process);
        }
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
