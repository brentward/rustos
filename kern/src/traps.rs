mod frame;
mod syndrome;
mod syscall;

pub mod irq;
pub use self::frame::TrapFrame;
pub use crate::console;
pub use crate::shell;


use pi::interrupt::{Controller, Interrupt};

use crate::console::kprintln;
use crate::IRQ;

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Kind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Source {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Info {
    source: Source,
    kind: Kind,
}

/// This function is called when an exception occurs. The `info` parameter
/// specifies the source and kind of exception that has occurred. The `esr` is
/// the value of the exception syndrome register. Finally, `tf` is a pointer to
/// the trap frame for the exception.
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame) {

    match info.kind {

        Kind::Synchronous => {
            let syndrome = Syndrome::from(esr);
            match syndrome {
                Syndrome::Brk(brk) => {
                    kprintln!("BRK: {}", brk);
                    shell::shell("!> ");
                    tf.elr += 4;

                }
                Syndrome::Svc(num) => handle_syscall(num, tf),
                _ => (),
            }
        }
        Kind::Irq => {
            let controller = Controller::new();
            for int in Interrupt::iter() {
                if controller.is_pending(*int) {
                    IRQ.invoke(*int, tf)
                }

            }


        }
        _ => (),
    }
}
