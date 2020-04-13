use crate::common::IO_BASE;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile, Reserved};

/// The base address for the ARM random number generator registers.
const RNG_REG_BASE: usize = IO_BASE + 0x104000;

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    CTRL: Volatile<u32>,
    STATUS: Volatile<u32>,
    DATA: ReadVolatile<u32>,
    _r0: Reserved<u32>,
    INT_MASK: Volatile<u32>,
}

/// The Raspberry Pi random number generator.
pub struct Rand {
    registers: &'static mut Registers,
}

impl Rand {
    pub fn new() -> Rand {
        let registers = unsafe { &mut *(RNG_REG_BASE as *mut Registers) };
        registers.STATUS.write(0x40000);
        registers.INT_MASK.or_mask(1);
        registers.CTRL.or_mask(1);
        while (registers.STATUS.read() >> 24) == 0 {
            unsafe { asm!("nop" :::: "volatile") };
        }

        Rand {
            registers,
        }
    }

    pub fn rand(&self, min: u32, max: u32) -> u32 {
        self.registers.DATA.read() % (max - min) + min
    }
}