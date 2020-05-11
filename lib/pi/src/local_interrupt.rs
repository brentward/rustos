use crate::common::IO_BASE;

use core::time::Duration;

use shim::const_assert_size;

use volatile::prelude::*;
use volatile::{Volatile, Reserved};
use aarch64::*;

// const INT_BASE: usize = 0x40000000;
const INT_BASE: usize = IO_BASE + 0x100_0000;

/// Core interrupt sources (QA7: 4.10)
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LocalInterrupt {
    CntpsIrq = 0,
    CntpnsIrq = 1,
    CnthpIrq = 2,
    CntvIrq = 3,
    Mailbox0 = 4,
    Mailbox1 = 5,
    Mailbox2 = 6,
    Mailbox3 = 7,
    Gpu = 8,
    Pmu = 9,
    AxiOutstanding = 10,
    LocalTimer = 11,
}

impl LocalInterrupt {
    pub const MAX: usize = 12;

    pub fn iter() -> impl Iterator<Item = LocalInterrupt> {
        (0..LocalInterrupt::MAX).map(|n| LocalInterrupt::from(n))
    }
}

impl From<usize> for LocalInterrupt {
    fn from(irq: usize) -> LocalInterrupt {
        use LocalInterrupt::*;
        match irq {
            0 => CntpsIrq,
            1 => CntpnsIrq,
            2 => CnthpIrq,
            3 => CntvIrq,
            4 => Mailbox0,
            5 => Mailbox1,
            6 => Mailbox2,
            7 => Mailbox3,
            8 => Gpu,
            9 => Pmu,
            10 => AxiOutstanding,
            11 => LocalTimer,
            _ => panic!("Unknown local irq: {}", irq),
        }
    }
}

/// BCM2837 Local Peripheral Registers (QA7: Chapter 4)
#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    control: Volatile<u32>,
    __r0: Reserved<u32>,
    core_timer_prescaler: Volatile<u32>,
    gpu_interrupts_routing: Volatile<u32>,
    pm_interrupts_routing_set: Volatile<u32>,
    pm_interrupts_routing_clear: Volatile<u32>,
    __r1: Reserved<u32>,
    core_timer_access_low: Volatile<u32>,
    core_timer_access_high: Volatile<u32>,
    local_interrupt_routing: Volatile<u32>,
    __r2: Reserved<u32>,
    axi_outstanding_counters: Volatile<u32>,
    axi_outstanding_irq: Volatile<u32>,
    local_timer_control_status: Volatile<u32>,
    local_timer_clear_reload: Volatile<u32>,
    __r3: Reserved<u32>,
    core_timer_interrupt_control: [Volatile<u32>; 4],
    core_mailboxes_interrupt_control: [Volatile<u32>; 4],
    core_irq_source: [Volatile<u32>; 4],
    core_fiq_source: [Volatile<u32>; 4],
}
const_assert_size!(Registers, 0x4000_0080 - 0x4000_0000);


pub struct LocalController {
    core: usize,
    registers: &'static mut Registers,
}

impl LocalController {
    /// Returns a new handle to the interrupt controller.
    pub fn new(core: usize) -> LocalController {
        let mut local_controller = LocalController {
            core: core,
            registers: unsafe { &mut *(INT_BASE as *mut Registers) },
        };
        // local_controller.enable_local_timer();
        local_controller
    }

    pub fn enable_local_timer(&mut self) {
        unsafe {
            CNTP_CTL_EL0.set(CNTP_CTL_EL0.get() | CNTP_CTL_EL0::ENABLE);
        }
        self.registers.core_timer_interrupt_control[self.core].write(1 << 1);
    }

    pub fn is_pending(&self, int: LocalInterrupt) -> bool {
        let index = int as usize;
        self.registers.core_irq_source[self.core].has_mask(1 << index)
    }

    pub fn tick_in(&mut self, t: Duration) {
        let timer_frequency = unsafe { CNTFRQ_EL0.get() };
        let ticks = (timer_frequency * t.as_nanos() as u64) / 1000000000;
        unsafe {
            CNTP_TVAL_EL0.set(CNTP_TVAL_EL0::TVAL & ticks);
            CNTP_CTL_EL0.set(CNTP_CTL_EL0.get() & !CNTP_CTL_EL0::IMASK);
        }
    }
}

pub fn local_tick_in(core: usize, t: Duration) {
    LocalController::new(core).tick_in(t);
}
