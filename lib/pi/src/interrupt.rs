use crate::common::IO_BASE;

use volatile::prelude::*;
use volatile::{Volatile, ReadVolatile, Reserved};

const INT_BASE: usize = IO_BASE + 0xb000 + 0x200;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interrupt {
    Timer1 = 1,
    Timer3 = 3,
    Usb = 9,
    Gpio0 = 49,
    Gpio1 = 50,
    Gpio2 = 51,
    Gpio3 = 52,
    Uart = 57,
}

impl Interrupt {
    pub const MAX: usize = 8;

    pub fn iter() -> impl Iterator<Item = Interrupt> {
        use Interrupt::*;
        [Timer1, Timer3, Usb, Gpio0, Gpio1, Gpio2, Gpio3, Uart]
            .iter()
            .map(|int| *int)
    }
}

impl From<usize> for Interrupt {
    fn from(irq: usize) -> Interrupt {
        use Interrupt::*;
        match irq {
            1 => Timer1,
            3 => Timer3,
            9 => Usb,
            49 => Gpio0,
            50 => Gpio1,
            51 => Gpio2,
            52 => Gpio3,
            57 => Uart,
            _ => panic!("Unknown irq: {}", irq),
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    pending_basic: Reserved<u32>,
    pending: [ReadVolatile<u32>; 2],
    fiq_control: Volatile<u32>,
    enable: [Volatile<u32>; 2],
    enable_basic: Reserved<u32>,
    disable: [Volatile<u32>; 2],
    disable_basic: Reserved<u32>,
}

/// An interrupt controller. Used to enable and disable interrupts as well as to
/// check if an interrupt is pending.
pub struct Controller {
    registers: &'static mut Registers,
}

impl Controller {
    /// Returns a new handle to the interrupt controller.
    pub fn new() -> Controller {
        Controller {
            registers: unsafe { &mut *(INT_BASE as *mut Registers) },
        }
    }

    /// Enables the interrupt `int`.
    pub fn enable(&mut self, int: Interrupt) {
        let index = int as u32;
        if index < 32 {
            self.registers.enable[0].or_mask(1 << index);
        } else {
            self.registers.enable[1].or_mask(1 << index - 32);
        }
    }

    /// Disables the interrupt `int`.
    pub fn disable(&mut self, int: Interrupt) {
        let index = int as u32;
        if index < 32 {
            self.registers.disable[0].write(1 << index);
        } else {
            self.registers.disable[1].write(1 << index - 32);
        }
    }

    /// Returns `true` if `int` is pending. Otherwise, returns `false`.
    pub fn is_pending(&self, int: Interrupt) -> bool {
        let index = int as u32;
        if index < 32 {
            self.registers.pending[0].has_mask(1 << index)
        } else {
            self.registers.pending[1].has_mask(1 << index - 32)
        }
    }

    /// Enables the interrupt as FIQ interrupt
    pub fn enable_fiq(&mut self, int: Interrupt) {
        self.disable(int);
        let index = int as u32;
        self.registers.fiq_control.write((1 << 7) | (index));
    }
}
