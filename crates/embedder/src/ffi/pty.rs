//! The one unsafe step of the pseudo-terminal: making the slave the controlling terminal of
//! the child, between fork and exec (`pre_exec`). Everything else is in `crate::pty`.

use std::os::fd::BorrowedFd;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Makes the child a session leader with its standard input (the pty slave) as the
/// controlling terminal: job control and `^C` then work as in any terminal.
pub fn controlling_terminal(command: &mut Command) {
    // SAFETY: the closure runs in the child between fork and exec. It only issues the
    // async-signal-safe syscalls setsid and ioctl(TIOCSCTTY) through rustix, allocates
    // nothing and touches no lock; fd 0 is the slave, set as stdin by the caller.
    unsafe {
        command.pre_exec(|| {
            rustix::process::setsid().map_err(errno)?;
            // SAFETY: fd 0 is open in the child (stdin is the pty slave) for the whole call.
            let stdin = BorrowedFd::borrow_raw(0);
            rustix::process::ioctl_tiocsctty(stdin).map_err(errno)?;
            Ok(())
        });
    }
}

/// An `io::Error` from an errno without allocating (safe between fork and exec).
fn errno(e: rustix::io::Errno) -> std::io::Error {
    std::io::Error::from_raw_os_error(e.raw_os_error())
}
