//! Contains the protocol for a control channel between workers and
//! the kleinhirn control process.

use anyhow::{Context, Result};
use nix::fcntl::{fcntl, FcntlArg};
use nix::unistd::close;
use std::os::unix::io::AsRawFd;
use tokio::io::BufStream;
use tokio::net::UnixStream;

/// Contains the worker's end of the control channel it uses to send
/// ack messages to the supervisor process.
#[derive(PartialEq, Clone, Eq)]
pub struct WorkerControlFD(i32, bool);

impl WorkerControlFD {
    fn new(fd: i32) -> Self {
        Self(fd, true)
    }

    /// Returns a base-10 representation of the worker control channel's FD number.
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    /// Returns the worker control channel's FD as a raw FD.
    pub unsafe fn into_inner(self) -> i32 {
        self.0
    }

    /// Closes our copy of the worker control channel. This must be
    /// called in order to prevent FD leaks. It is called as part of
    /// the destructor.
    pub fn close(&mut self) -> Result<()> {
        if self.1 {
            close(self.0).context("closing the dup'ed FD")?;
            self.1 = false;
        }
        Ok(())
    }
}

impl Drop for WorkerControlFD {
    fn drop(&mut self) {
        self.close()
            .expect("Couldn't close our copy of the worker control FD on drop");
    }
}

/// Opens a streaming UNIX domain socket pair that can be passed to a
/// worker, and returns the FD number of the writable end, and the
/// readable end of the pair.
pub fn worker_status_stream() -> Result<(WorkerControlFD, BufStream<UnixStream>)> {
    let (ours, theirs) = std::os::unix::net::UnixStream::pair()
        .context("Could not initialize preloader unix socket pair")?;
    let their_fd = fcntl(theirs.as_raw_fd(), FcntlArg::F_DUPFD(theirs.as_raw_fd()))
        .context("Could not clear CLOEXEC from the status pipe")?;
    close(theirs.as_raw_fd()).context("closing the remote FD")?;
    let socket = UnixStream::from_std(ours).context("unable to setup UNIX stream")?;
    let reader = BufStream::new(socket);
    Ok((WorkerControlFD::new(their_fd), reader))
}
