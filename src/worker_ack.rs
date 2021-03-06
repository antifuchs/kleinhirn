//! Contains the protocol for a control channel between workers and
//! the kleinhirn control process.

use anyhow::{Context, Result};
use futures::io::{BufReader, BufWriter};
use nix::fcntl::{fcntl, FcntlArg};
use nix::unistd::close;
use serde::Deserialize;
use smol::Async;
use std::{
    fmt,
    os::unix::{io::AsRawFd, net::UnixStream},
};

/// Contains the worker's end of the control channel it uses to send
/// ack messages to the supervisor process.
#[derive(PartialEq, Clone, Debug, Eq)]
pub struct WorkerControlFD(i32, bool);

impl WorkerControlFD {
    fn new(fd: i32) -> Self {
        Self(fd, true)
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

impl fmt::Display for WorkerControlFD {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Drop for WorkerControlFD {
    fn drop(&mut self) {
        self.close()
            .expect("Couldn't close our copy of the worker control FD on drop");
    }
}

pub(crate) type ControlChannel = BufWriter<BufReader<Async<UnixStream>>>;

/// Opens a streaming UNIX domain socket pair that can be passed to a
/// worker, and returns the FD number of the writable end, and the
/// readable end of the pair.
pub fn worker_status_stream() -> Result<(WorkerControlFD, ControlChannel)> {
    let (ours, theirs_with_cloexec) =
        UnixStream::pair().context("Could not initialize preloader unix socket pair")?;
    let their_fd = fcntl(
        theirs_with_cloexec.as_raw_fd(),
        FcntlArg::F_DUPFD(theirs_with_cloexec.as_raw_fd()),
    )
    .context("Could not clear CLOEXEC from the status pipe")?;

    close(theirs_with_cloexec.as_raw_fd()).context("closing the remote FD")?;
    Ok((
        WorkerControlFD::new(their_fd),
        BufWriter::new(BufReader::new(
            Async::new(ours).context("Could not convert our FD to async")?,
        )),
    ))
}

/// The vocabulary of messages that any kind of process can send to
/// Einhorn. This is a strict subset of the preloader message
/// vocabulary.
#[derive(PartialEq, Debug, Clone, Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub enum WorkerControlMessage {
    /// A worker process with the given ID has finished initializing
    /// and is now able to do work. The `id` field must correspond to
    /// the worker ID string given to the worker.
    Ack { id: String },
}
