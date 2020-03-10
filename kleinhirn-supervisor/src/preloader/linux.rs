#![cfg(target_os = "linux")]

use super::Preloader;
use anyhow::{Context, Result};
use nix::fcntl::{fcntl, FcntlArg};
use nix::unistd::close;
use slog_scope::debug;
use std::{os::unix::io::AsRawFd, path::Path, process::Command};
use tokio::io::BufStream;
use tokio::net::UnixStream;

impl Preloader {
    /// Constructs the ruby preloader, starts it and waits until the code is loaded.
    pub fn for_ruby(gemfile: &Path, load: &Path, start_expression: &str) -> Result<Preloader> {
        prctl::set_child_subreaper(true)
            .map_err(|code| anyhow::anyhow!("Unable to set subreaper status. Status {:?}", code))?;

        let (ours, theirs) = std::os::unix::net::UnixStream::pair()
            .context("Could not initialize preloader unix socket pair")?;
        let their_fd = fcntl(theirs.as_raw_fd(), FcntlArg::F_DUPFD(theirs.as_raw_fd()))
            .context("Could not clear CLOEXEC from the status pipe")?;
        close(theirs.as_raw_fd()).context("closing the remote FD")?;

        let theirs_str = their_fd.to_string();
        let mut cmd = Command::new("bundle");
        cmd.args(&["exec", "--gemfile"])
            .arg(gemfile.as_os_str())
            .args(&[
                "--keep-file-descriptors",
                "--",
                "kleinhirn_loader",
                "--status-fd",
                &theirs_str,
                "-e",
                start_expression,
                "-r",
            ])
            .arg(load.as_os_str());
        debug!("running preloader"; "cmd" => ?cmd);
        let child = cmd.spawn().context("spawning kleinhirn_loader")?;
        let socket = UnixStream::from_std(ours).context("unable to setup UNIX stream")?;
        let reader = BufStream::new(socket);
        debug!("child running"; "pid" => ?child.id());
        // close the socket we passed to our children:
        close(their_fd).context("closing the dup'ed FD")?;

        Ok(Preloader {
            control_channel: reader,
            pid: child.id(),
        })
    }
}
