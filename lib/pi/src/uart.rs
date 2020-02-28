use core::fmt;
use core::time::Duration;

use shim::const_assert_size;
use shim::io;

use volatile::prelude::*;
use volatile::{ReadVolatile, Reserved, Volatile};

use crate::common::IO_BASE;
use crate::gpio::{Function, Gpio};
use crate::timer;

/// The base address for the `MU` registers.
const MU_REG_BASE: usize = IO_BASE + 0x215040;

/// The `AUXENB` register from page 9 of the BCM2837 documentation.
const AUX_ENABLES: *mut Volatile<u8> = (IO_BASE + 0x215004) as *mut Volatile<u8>;

/// Enum representing bit fields of the `AUX_MU_LSR_REG` register.
#[repr(u8)]
enum LsrStatus {
    DataReady = 1,
    TxAvailable = 1 << 5,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    IO: Volatile<u8>,
    __r0: Reserved<u8>,
    __r1: Reserved<u8>,
    __r2: Reserved<u8>,
    IER: Volatile<u8>,
    __r3: Reserved<u8>,
    __r4: Reserved<u8>,
    __r5: Reserved<u8>,
    IIR: Volatile<u8>,
    __r06: Reserved<u8>,
    __r7: Reserved<u8>,
    __r8: Reserved<u8>,
    LCR: Volatile<u8>,
    __r9: Reserved<u8>,
    __r10: Reserved<u8>,
    __r11: Reserved<u8>,
    MCR: Volatile<u8>,
    __r12: Reserved<u8>,
    __r13: Reserved<u8>,
    __r14: Reserved<u8>,
    LSR: ReadVolatile<u8>,
    __r15: Reserved<u8>,
    __r16: Reserved<u8>,
    __r17: Reserved<u8>,
    MSR: ReadVolatile<u8>,
    __r18: Reserved<u8>,
    __r19: Reserved<u8>,
    __r20: Reserved<u8>,
    SCRATCH: Volatile<u8>,
    __r21: Reserved<u8>,
    __r22: Reserved<u8>,
    __r23: Reserved<u8>,
    CNTL: Volatile<u8>,
    __r24: Reserved<u8>,
    __r25: Reserved<u8>,
    __r26: Reserved<u8>,
    STAT: ReadVolatile<u32>,
    BAUD: Volatile<u16>,
}

const_assert_size!(Registers, 0x7E21506C - 0x7E215040);

/// The Raspberry Pi's "mini UART".
pub struct MiniUart {
    registers: &'static mut Registers,
    timeout: Option<Duration>,
}

impl MiniUart {
    /// Initializes the mini UART by enabling it as an auxiliary peripheral,
    /// setting the data size to 8 bits, setting the BAUD rate to ~115200 (baud
    /// divider of 270), setting GPIO pins 14 and 15 to alternative function 5
    /// (TXD1/RDXD1), and finally enabling the UART transmitter and receiver.
    ///
    /// By default, reads will never time out. To set a read timeout, use
    /// `set_read_timeout()`.
    pub fn new() -> MiniUart {
        let registers = unsafe {
            // Enable the mini UART as an auxiliary device.
            (*AUX_ENABLES).or_mask(1);
            &mut *(MU_REG_BASE as *mut Registers)
        };

        registers.LCR.write(0b11);
        registers.BAUD.write(270);
        Gpio::new(14).into_alt(Function::Alt5);
        Gpio::new(15).into_alt(Function::Alt5);
        registers.CNTL.write(0b11);
        registers.IIR.write(0b11 << 1);

        MiniUart {
            registers,
            timeout: None,
        }
    }

    /// Set the read timeout to `t` duration.
    pub fn set_read_timeout(&mut self, t: Duration) {
        self.timeout = Some(t);
    }

    /// Write the byte `byte`. This method blocks until there is space available
    /// in the output FIFO.
    pub fn write_byte(&mut self, byte: u8) {
        while !self.registers.LSR.has_mask(LsrStatus::TxAvailable as u8) {
            continue
        }
        self.registers.IO.write(byte);
    }

    /// Returns `true` if there is at least one byte ready to be read. If this
    /// method returns `true`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately. This method does not block.
    pub fn has_byte(&self) -> bool {
        self.registers.LSR.has_mask(LsrStatus::DataReady as u8)
    }

    /// Blocks until there is a byte ready to read. If a read timeout is set,
    /// this method blocks for at most that amount of time. Otherwise, this
    /// method blocks indefinitely until there is a byte to read.
    ///
    /// Returns `Ok(())` if a byte is ready to read. Returns `Err(())` if the
    /// timeout expired while waiting for a byte to be ready. If this method
    /// returns `Ok(())`, a subsequent call to `read_byte` is guaranteed to
    /// return immediately.
    pub fn wait_for_byte(&self) -> Result<(), ()> {
        match self.timeout {
            Some(timeout) => {
                let timeout_time = timer::current_time() + timeout;
                while timer::current_time() < timeout_time {
                    if self.has_byte() {
                        return Ok(())
                    }
                }
                Err(())
            }
            None => {
                if self.has_byte() {
                    Ok(())
                } else {
                    Err(())
                }
            }
        }
    }

    /// Reads a byte. Blocks indefinitely until a byte is ready to be read.
    pub fn read_byte(&mut self) -> u8 {
        while !self.has_byte() {
            continue
        }
        self.registers.IO.read()
    }
}

impl fmt::Write for MiniUart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        let mut buf = [0; 4];
        for &byte in c.encode_utf8(&mut buf).as_bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

mod uart_io {
    use super::io;
    use super::MiniUart;
    use volatile::prelude::*;

    impl io::Read for MiniUart {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
            let wait = self.wait_for_byte();
            match wait {
                Ok(_) => {
                    let mut byte_count = 0;
                    while self.has_byte() && byte_count < buf.len() {
                        buf[byte_count] = self.read_byte();
                        byte_count += 1;
                    }
                    Ok(byte_count)
                }
                Err(_) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "MiniUart read timed out",
                ))
            }
        }
    }

    impl io::Write for MiniUart {
        fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
            let mut byte_count= 0;
            for byte in buf {
                self.write_byte(*byte);
                byte_count += 1;
            }
            Ok(byte_count)
        }

        fn flush(&mut self) -> Result<(), io::Error> {
            Ok(())
        }
    }
}
