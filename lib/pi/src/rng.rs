use crate::common::IO_BASE;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile};

/// The base address for the ARM random number generator registers.
const RNG_REG_BASE: usize = IO_BASE + 0x104000;

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    CTRL: Volatile<u32>,
    STATUS: Volatile<u32>,
    DATA: ReadVolatile<u32>,
    FF_THRESHOLD: Volatile<u32>,
    INT_MASK: Volatile<u32>,
}

/// The Raspberry Pi random number generator.
pub struct Rng {
    registers: &'static mut Registers,
}

impl Rng {
    const RBGEN: u32 = 0x1;
    const WARMUP_COUNT: u32 = 0x40000;
    const INT_OFF: u32 = 0x1;

    pub fn new() -> Rng {
        let registers = unsafe { &mut *(RNG_REG_BASE as *mut Registers) };
        registers.INT_MASK.or_mask(Rng::INT_OFF);
        registers.STATUS.write(Rng::WARMUP_COUNT);
        registers.CTRL.or_mask(Rng::RBGEN);
        Rng {
            registers,
        }
    }

    pub fn rand(&self, min: u32, max: u32) -> u32 {
        while (self.registers.STATUS.read() >> 24) == 0 {
            unsafe { asm!("nop" :::: "volatile") };
        }
        self.registers.DATA.read() % (max - min) + min
    }

    pub fn r_rand(&self) -> u32 {
        while (self.registers.STATUS.read() >> 24) == 0 {
            unsafe { asm!("nop" :::: "volatile") };
        }
        self.registers.DATA.read()
    }

    pub fn entropy(&self) -> u32 {
        self.registers.STATUS.read()
    }
}