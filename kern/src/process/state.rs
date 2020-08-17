use core::fmt;

use alloc::boxed::Box;

use crate::process::Process;

/// Type of a function used to determine if a process is ready to be scheduled
/// again. The scheduler calls this function when it is the process's turn to
/// execute. If the function returns `true`, the process is scheduled. If it
/// returns `false`, the process is not scheduled, and this function will be
/// called on the next time slice.
pub type EventPollFn = Box<dyn FnMut(&mut Process) -> bool + Send>;

/// The scheduling state of a process.
pub enum State {
    // /// The process is being initialized.
    // Starting,
    /// The process is ready to be scheduled.
    Ready,
    /// The process is waiting on an event to occur before it can be scheduled.
    Waiting(EventPollFn),
    // /// The processis waiting for a pid to terminate.
    // WaitFor(u64),
    /// The process is currently running.
    Running,
    // /// The process is being held indefinitely.
    // Holding,
    /// The process is currently dead (ready to be reclaimed).
    Dead,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            // State::Starting => write!(f, "State::Starting"),
            State::Ready => write!(f, "State::Ready"),
            State::Running => write!(f, "State::Running"),
            State::Waiting(_) => write!(f, "State::Waiting"),
            // State::Holding => write!(f, "State::Holding"),
            State::Dead => write!(f, "State::Dead"),
            // State::WaitFor(pid) => write!(f, "State::WaitFor(pid: {})", pid),
        }
    }
}
