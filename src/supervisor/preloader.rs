use slog_scope::debug;

use crate::configuration;
use anyhow::{Context, Error, Result};
use async_trait::async_trait;
use closefds::close_fds_on_exec;
use serde::{Deserialize, Serialize};
use slog_scope::info;
use std::{
    os::unix::{io::AsRawFd, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream};
use tokio::net::UnixStream;
use uuid::Uuid;

/// Allows control over worker processes
#[async_trait]
pub trait ProcessControl {
    type Message;

    /// Returns success when the process controller is
    /// initialized. This is a no-op on regular programs, but a
    /// preloader will resolve here when the code is loaded.
    async fn initialize(&mut self) -> Result<()>;

    async fn spawn_process(&mut self) -> Result<String>;

    async fn until_ready(&mut self) -> Result<String>;

    async fn next_message(&mut self) -> Result<Self::Message>;
}

#[derive(PartialEq, Debug, Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub enum PreloaderMessage {
    /// The preloader is loading a file
    Loading {
        /// Path of the file being loaded
        file: PathBuf,
    },

    /// The preloader is done loading all files and standing by to spawn new processes.
    Ready,

    /// The preloader encountered an error with the command that was sent.
    Error {
        message: String,
        error: Option<String>,
    },

    /// A problem occurred spawning a worker.
    Failed { id: String, message: String },

    /// A worker process has been launched and is initializing.
    Launched { id: String, pid: i32 },

    /// A worker process has finished initializing and is now running.
    Ack { id: String },
}

#[derive(PartialEq, Debug, Serialize)]
#[serde(tag = "op")]
#[serde(rename_all = "snake_case")]
enum PreloaderRequest {
    Spawn { id: String },
}

#[derive(Debug)]
pub struct Preloader {
    control_channel: BufStream<UnixStream>,
    pid: u32,
}

impl Preloader {
    pub fn for_ruby(gemfile: &Path, load: &Path, start_expression: &str) -> Result<Preloader> {
        let (ours, theirs) = std::os::unix::net::UnixStream::pair()
            .context("Could not initialize preloader unix socket pair")?;
        let theirs_str = theirs.as_raw_fd().to_string();
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

        unsafe {
            cmd.pre_exec(close_fds_on_exec(vec![0, 1, 2, theirs.as_raw_fd()])?);
        }
        debug!("running preloader"; "cmd" => format!("{:?}", cmd));
        let child = cmd.spawn().context("spawning kleinhirn_loader")?;
        let socket = UnixStream::from_std(ours).context("unable to setup UNIX stream")?;
        let reader = BufStream::new(socket);
        debug!("child running"; "pid" => format!("{:?}", child.id()));
        Ok(Preloader {
            control_channel: reader,
            pid: child.id(),
        })
    }

    async fn send_message(&mut self, msg: &PreloaderRequest) -> Result<()> {
        let mut msg = serde_json::to_vec(msg)?;
        info!("sending"; "msg" => String::from_utf8(msg.clone()).unwrap());
        msg.push('\n' as u8);
        self.control_channel.write_all(&msg).await?;
        self.control_channel.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl ProcessControl for Preloader {
    async fn initialize(&mut self) -> Result<()> {
        loop {
            let msg = self.next_message().await?;
            match msg {
                PreloaderMessage::Loading { file } => {
                    debug!("loading"; "file" => file.to_str());
                }
                PreloaderMessage::Ready => {
                    debug!("ready");
                    return Ok(());
                }
                other => {
                    return Err(Error::msg(format!(
                        "Unexpected status from preloader: {:?}",
                        other
                    )));
                }
            }
        }
    }

    async fn spawn_process(&mut self) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        self.send_message(&PreloaderRequest::Spawn { id: id.to_string() })
            .await?;
        Ok(id)
    }

    async fn until_ready(&mut self) -> Result<String> {
        todo!("need to figure out how to wait")
    }

    type Message = PreloaderMessage;

    async fn next_message(&mut self) -> Result<Self::Message> {
        let mut line = String::new();
        self.control_channel.read_line(&mut line).await?;
        let msg: PreloaderMessage = serde_json::from_str(&line)?;
        Ok(msg)
    }
}

pub struct ForkExec(configuration::Program);

impl ForkExec {
    pub fn for_program(p: &configuration::Program) -> Result<ForkExec> {
        // TODO: do some error checking
        Ok(ForkExec(p.clone()))
    }
}

#[async_trait]
impl ProcessControl for ForkExec {
    async fn initialize(&mut self) -> Result<()> {
        // No preparation necessary - we're ready to launch immediately.
        Ok(())
    }
    async fn spawn_process(&mut self) -> Result<String> {
        todo!("no idea yet how to spawn!")
    }

    async fn until_ready(&mut self) -> Result<String> {
        // Nothing to do for fork/exec programs here, we just assume
        // they're ready immediately.

        // TODO: maybe we can in fact do worker acking, but ehhh for now.
        Ok("".to_string())
    }
    type Message = ();
    async fn next_message(&mut self) -> Result<Self::Message> {
        Ok(())
    }
}
