use anyhow::{Context, Result};
use futures::Stream;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;
use parking_lot::Mutex;
use slog_scope::{debug, info};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task;
use tokio::{io, io::AsyncReadExt, net::UnixStream};

/// Sets the current process as the "child subreaper", and sets up a SIGCHLD handler for
/// asynchronously waking up & reaping all eligible children. The reaped children's PIDs are
/// returned in a stream.
pub fn setup_child_exit_handler() -> Result<Zombies> {
    // TODO: use prctl so set up the subreaper reaper state

    let (read, write) = std::os::unix::net::UnixStream::pair()
        .context("Could not initialize signal handler socket pair")?;
    signal_hook::pipe::register(signal_hook::SIGCHLD, write)
        .context("registering sigchld handler")?;
    Ok(Zombies {
        socket: UnixStream::from_std(read).context("unable to setup UNIX stream")?,
    })
}

pub struct Zombies {
    socket: UnixStream,
}

impl Stream for Zombies {
    type Item = Result<Pid>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
    ) -> task::Poll<Option<Result<Pid>>> {
        use task::Poll::*;

        let mut buf = vec![0u8; 256];
        loop {
            let mut read = self.socket.read_buf(&mut buf);
            let rp = Pin::new(&mut read);
            match rp.poll(cx) {
                NotReady => {
                    // we'll get woken up when the next SIGCHLD comes in.
                    break;
                }
                // We're definitely ready to reap children; but clear out any pending bytes from the
                // pipe before we start (this also schedules this stream to be polled when bytes are
                // ready).
                Ready(Ok(_)) => (),
                Ready(Err(e)) => {
                    return task::Poll::Ready(Some(Err(e.into())));
                }
            }
        }

        let flags = WaitPidFlag::empty() | WaitPidFlag::WNOHANG; // TODO: use WEXITED on linux
        use nix::sys::wait::WaitStatus::*;
        match waitpid(None, Some(flags)) {
            Ok(Exited(pid, _)) => {
                info!("detected exited process"; "pid" => pid.as_raw());
                // At least one child is ready to be reaped; return the first one and then
                // schedule this for waking up again:
                task::Poll::Ready(Some(Ok(pid)))
            }
            Ok(StillAlive) => task::Poll::Pending,

            // peaceful: we have no children.
            Err(ECHILD) => task::Poll::Pending,

            // any other error: probably not great.
            Err(e) => task::Poll::Ready(Some(Err(e.into()))),
            _ => task::Poll::Pending,
        }
    }
}
