use aarch64::ESR_EL1;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Fault {
    AddressSize,
    Translation,
    AccessFlag,
    Permission,
    Alignment,
    TlbConflict,
    Other(u8),
}

impl From<u32> for Fault {
    fn from(val: u32) -> Fault {
        match (val & 0b111111) as u8 {
            0b000000..=0b000011 => Fault::AddressSize,
            0b000100..=0b000111 => Fault::Translation,
            0b001001..=0b001011 => Fault::AccessFlag,
            0b001101..=0b001111 => Fault::Permission,
            0b100001 => Fault::Alignment,
            0b110000 => Fault::TlbConflict,
            other => Fault::Other(other),
        }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Syndrome {
    Unknown,
    WfiWfe,
    SimdFp,
    IllegalExecutionState,
    Svc(u16),
    Hvc(u16),
    Smc(u16),
    MsrMrsSystem,
    InstructionAbort { kind: Fault, level: u8 },
    PCAlignmentFault,
    DataAbort { kind: Fault, level: u8 },
    SpAlignmentFault,
    TrappedFpu,
    SError,
    Breakpoint,
    Step,
    Watchpoint,
    Brk(u16),
    Other(u32),
}

/// Converts a raw syndrome value (ESR) into a `Syndrome` (ref: D1.10.4).
impl From<u32> for Syndrome {
    fn from(esr: u32) -> Syndrome {
        use self::Syndrome::*;
        match esr >> 26 {
            0b000000 => Syndrome::Unknown,
            0b000001 => Syndrome::WfiWfe,
            0b000111 => Syndrome::SimdFp,
            0b001110 => Syndrome::IllegalExecutionState,
            0b010001 | 0b010101 => Syndrome::Svc((esr & 0xFFFF) as u16),
            0b010010 | 0b010110 => Syndrome::Hvc((esr & 0xFFFF) as u16),
            0b010011 | 0b010111 => Syndrome::Smc((esr & 0xFFFF) as u16),
            0b011000 => Syndrome::MsrMrsSystem,
            0b100000 | 0b100001 => Syndrome::InstructionAbort {
                kind: Fault::from((esr)),
                level: (esr & 0b11) as u8,
            },
            0b100010 => Syndrome::PCAlignmentFault,
            0b100100 | 0b100101 => Syndrome::DataAbort {
                kind: Fault::from((esr)),
                level: (esr & 0b11) as u8,
            },
            0b100110 => Syndrome::SpAlignmentFault,
            0b101000 | 0b101100 => Syndrome::TrappedFpu,
            0b101111 => Syndrome::SError,
            0b110000 | 0b110001 => Syndrome::Breakpoint,
            0b110010 | 0b110011 => Syndrome::Step,
            0b110100 | 0b110101 => Syndrome::Watchpoint,
            0b111000 | 0b111100 => Syndrome::Brk((esr & 0xFFFF) as u16),
            other_esr => Syndrome::Other(other_esr),
        }
    }
}
