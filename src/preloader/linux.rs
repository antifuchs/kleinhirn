#![cfg(target_os = "linux")]

use super::Preloader;
use crate::worker_ack;
use anyhow::{Context, Result};
use slog_scope::debug;
use std::{path::Path, process::Command};

impl Preloader {
    /// Constructs the ruby preloader, starts it and waits until the code is loaded.
    pub fn for_ruby(gemfile: &Path, load: &Path, start_expression: &str) -> Result<Preloader> {
        prctl::set_child_subreaper(true)
            .map_err(|code| anyhow::anyhow!("Unable to set subreaper status. Status {:?}", code))?;
        let (their_fd, control_channel) = worker_ack::worker_status_stream()?;
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
        debug!("child running"; "pid" => ?child.id());

        Ok(Preloader {
            control_channel,
            pid: child.id(),
        })
    }
}
