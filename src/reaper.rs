use anyhow::{Context, Result};
use nix::errno::Errno;
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::Pid;
use slog_scope::debug;
use smol::Async;
use std::{io::Read, os::unix::net::UnixStream};

/// Sets the current process as the "child subreaper", and sets up a SIGCHLD handler for
/// asynchronously waking up & reaping all eligible children. The reaped children's PIDs are
/// returned in a stream.
pub fn setup_child_exit_handler() -> Result<Zombies> {
    let (read, write) =
        UnixStream::pair().context("Could not initialize signal handler socket pair")?;
    signal_hook::pipe::register(signal_hook::SIGCHLD, write)
        .context("registering sigchld handler")?;
    Ok(Zombies {
        socket: Async::new(read)?,
    })
}

pub struct Zombies {
    socket: Async<UnixStream>,
}

impl Zombies {
    pub async fn reap(&mut self) -> Result<Pid> {
        let flags = WaitPidFlag::empty() | WaitPidFlag::WNOHANG; // TODO: use WEXITED on linux

        use nix::sys::wait::WaitStatus::*;
        loop {
            match waitpid(None, Some(flags)) {
                Ok(Exited(pid, _)) | Ok(Signaled(pid, _, _)) => {
                    // At least one child is ready to be reaped; return the first one and then
                    // schedule this for waking up again:
                    return Ok(pid);
                }
                Ok(StillAlive) |
                // peaceful: we have no children.
                Err(nix::Error::Sys(Errno::ECHILD)) => {
                }

                // any other error: probably not great.
                Err(e) => {return Err(e.into());}

                // Anything else is a status change we don't care about. On to the next one:
                e => {
                    debug!("weird process change detected that we'll ignore"; "change" => ?e);
                }
            }

            // No processes are ready to be reaped, schedule us to get
            // woken up when the next one terminates:
            let mut buf = vec![0u8; 256];
            self.socket
                .read_with_mut(|io| io.read(&mut buf))
                .await
                .context("Failed to read from zombie notification pipe")?;
        }
    }
}
