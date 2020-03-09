use nix::fcntl::{fcntl, FcntlArg};
use slog_scope::debug;

use self::machine::PreloaderState;
use crate::process_control::{Message, ProcessControl};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use slog_scope::info;
use std::{
    collections::HashMap,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    process::Command,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufStream};
use tokio::net::UnixStream;

mod logging;
mod machine;

#[derive(PartialEq, Debug, Clone, Deserialize)]
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
    Launched { id: String, pid: u32 },

    /// A worker process has finished initializing and is now running.
    Ack { id: String },

    /// Some message that the preloader or worker wants us to log.
    Log {
        level: LogLevel,
        msg: String,
        #[serde(flatten)]
        kv: HashMap<String, String>,
    },
}

#[derive(PartialEq, Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
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
    /// Constructs the ruby preloader, starts it and waits until the code is loaded.
    pub fn for_ruby(gemfile: &Path, load: &Path, start_expression: &str) -> Result<Preloader> {
        let (ours, theirs) = std::os::unix::net::UnixStream::pair()
            .context("Could not initialize preloader unix socket pair")?;
        let their_fd = fcntl(theirs.as_raw_fd(), FcntlArg::F_DUPFD(theirs.as_raw_fd()))
            .context("Could not clear CLOEXEC from the status pipe")?;

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
        Ok(Preloader {
            control_channel: reader,
            pid: child.id(),
        })
    }

    async fn send_message(&mut self, msg: &PreloaderRequest) -> Result<()> {
        let mut msg = serde_json::to_vec(msg)?;
        info!("sending"; "msg" => String::from_utf8(msg.clone()).unwrap());
        msg.push(b'\n');
        self.control_channel.write_all(&msg).await?;
        self.control_channel.flush().await?;
        Ok(())
    }

    async fn next_preloader_message(&mut self) -> Result<PreloaderMessage> {
        loop {
            let mut line = String::new();
            self.control_channel.read_line(&mut line).await?;
            if let Some(msg) = logging::translate_message(serde_json::from_str(&line)?) {
                return Ok(msg);
            }
        }
    }
}

#[async_trait]
impl ProcessControl for Preloader {
    async fn initialize(&mut self) -> Result<()> {
        let mut state = PreloaderState::starting();
        while let PreloaderState::Starting(_) | PreloaderState::Loading(_) = state {
            let msg = self.next_preloader_message().await?;
            state = state.on_preloader_message(msg);
        }
        match state {
            PreloaderState::Ready(_) => Ok(()),
            state => {
                bail!("Unexpected preloader state {:?}", state);
            }
        }
    }

    async fn spawn_process(&mut self) -> Result<String> {
        let id = self.generate_id();
        self.send_message(&PreloaderRequest::Spawn { id: id.to_string() })
            .await?;
        Ok(id)
    }

    async fn next_message(&mut self) -> Result<Message> {
        use PreloaderMessage::*;
        match self.next_preloader_message().await? {
            Launched { id, pid } => Ok(Message::Launched { id, pid }),
            Ack { id } => Ok(Message::Ack { id }),
            msg => {
                bail!("Unexpected preloader message {:?}", msg);
            }
        }
    }
}
