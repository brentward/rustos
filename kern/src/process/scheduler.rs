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
        enable_fiq_interrupt();
        let proc_id = self.switch_to(&mut tf);
        let x_regs_ptr = tf.x.as_ptr() as usize;
        let q_regs_ptr = tf.q.as_ptr() as usize;
        info!("SCHEDULER::start() core-{}/first-process={}", core, proc_id);
        trace!("SCHEDULER::start() core-{}/tf={:?}", core, tf);
        unsafe {
            asm!(
                "mrs x0, MPIDR_EL1 // calcluate core stack: store register containing core affinity in x0
                and x0, x0, #0xff // mask to get core_n
                mov x1, $3 // move KERNEL_STACK_BASE into x1: 0x80_000
                mov x2, $4 // move KERNEL_STACK_SIZE into x2: 0x10_000
                msub x0, x0, x2, x1 // calculate stack for core and store in x0 (KERNEL_STACK_BASE - (core_n * KERNEL_STACK_SIZE)
                str $1, [x0] // store address of tf.x into address at x0 (the calulated stack pointer)
                str $2, [x0, #-8] // store address of tf.q into the address at x0-8
                mov SP, $0 // move address of tf into the stack pointer
                bl context_restore // restore tf to prepare it to run
                mrs x0, MPIDR_EL1 // repeat steps to calculate core stack pointer
                and x0, x0, #0xff // this is required because all of the registers are now set to
                mov x1, $3 // the tf so our calcuated SP is overwriten
                mov x2, $4
                msub x0, x0, x2, x1
                mov SP, x0 // set the stack pointer to the new calculated core stack pointer
                ldr x1, [x0] // load the value in the address at x0 into x1, this contains the address tf.x
                ldr x2, [x0, #-8] // load the value in the address at x0 - 8 into x2, this contains the address of tf.q
                ldr q0, [x2] // restore q registers from the address of tf.q
                ldr q1, [x2, #16]
                ldr q2, [x2, #32]
                ldr q3, [x2, #48]
                ldr q4, [x2, #64]
                ldr q5, [x2, #80]
                ldr q6, [x2, #96]
                ldr q7, [x2, #112]
                ldr q8, [x2, #128]
                ldr q9, [x2, #144]
                ldr q10, [x2, #160]
                ldr q11, [x2, #176]
                ldr q12, [x2, #192]
                ldr q13, [x2, #208]
                ldr q14, [x2, #224]
                ldr q15, [x2, #240]
                ldr q16, [x2, #256]
                ldr q17, [x2, #272]
                ldr q18, [x2, #288]
                ldr q19, [x2, #304]
                ldr q20, [x2, #320]
                ldr q21, [x2, #336]
                ldr q22, [x2, #352]
                ldr q23, [x2, #368]
                ldr q24, [x2, #384]
                ldr q25, [x2, #400]
                ldr q26, [x2, #416]
                ldr q27, [x2, #432]
                ldr q28, [x2, #448]
                ldr q29, [x2, #464]
                ldr q30, [x2, #480]
                ldr q31, [x2, #496]
                ldr x0, [x1] // restore x registers from the address of tf.x
                ldr x2, [x1, #16] // skip restoring x1 until last since it contains the address we are loading from
                ldr x3, [x1, #24]
                ldr x4, [x1, #32]
                ldr x5, [x1, #40]
                ldr x6, [x1, #48]
                ldr x7, [x1, #56]
                ldr x8, [x1, #64]
                ldr x9, [x1, #72]
                ldr x10, [x1, #80]
                ldr x11, [x1, #88]
                ldr x12, [x1, #96]
                ldr x13, [x1, #104]
                ldr x14, [x1, #112]
                ldr x15, [x1, #120]
                ldr x16, [x1, #128]
                ldr x17, [x1, #136]
                ldr x18, [x1, #144]
                ldr x19, [x1, #152]
                ldr x20, [x1, #160]
                ldr x21, [x1, #168]
                ldr x22, [x1, #176]
                ldr x23, [x1, #184]
                ldr x24, [x1, #192]
                ldr x25, [x1, #200]
                ldr x26, [x1, #208]
                ldr x27, [x1, #216]
                ldr x28, [x1, #224]
                ldr x29, [x1, #232]
                ldr lr, [x1, #240]
                ldr x1, [x1, #8]
                eret"
                :: "r"(&tf as *const TrapFrame),  "r"(x_regs_ptr), "r"(q_regs_ptr),
                    "i"(KERN_STACK_BASE), "i"(KERN_STACK_SIZE)
                : "x0", "x1", "x2"
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
        USB.start_kernel_timer(Duration::from_secs(1), Some(poll_ethernet));
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
//pub type TKernelTimerHandler = Option<
//     unsafe extern "C" fn(hTimer: TKernelTimerHandle, pParam: *mut c_void, pContext: *mut c_void),
// >;
/// Poll the ethernet driver and re-register a timer handler using
/// `Usb::start_kernel_timer`.
extern "C" fn poll_ethernet(_: TKernelTimerHandle, _: *mut c_void, _: *mut c_void) {
    ETHERNET.poll(Instant::from_millis(timer::current_time().as_millis() as i64));
    let delay = ETHERNET.poll_delay(
        Instant::from_millis(timer::current_time().as_millis() as i64)
    );
    USB.start_kernel_timer(delay, Some(poll_ethernet));
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
