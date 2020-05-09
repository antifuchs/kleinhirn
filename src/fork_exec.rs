use crate::configuration;
use crate::{
    process_control::{Message, ProcessControl},
    worker_ack,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use slog_scope::debug;
use std::env::current_dir;
use std::{collections::HashMap, process::Command};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufStream};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use worker_ack::WorkerControlMessage;

#[derive(Debug, Clone, PartialEq)]
enum Action {
    Fork(String, u32),
    Ack(String),
}

pub struct ForkExec {
    program: configuration::Program,
    sender: Sender<Action>,
    receiver: Receiver<Action>,
}

#[derive(Error, Debug, PartialEq)]
#[error("worker process has died or closed the control channel")]
pub struct WorkerDied;

impl ForkExec {
    pub fn for_program(p: &configuration::Program) -> Result<ForkExec> {
        // TODO: do some error checking - validate that the program can be found and such?
        let (sender, receiver) = channel(20);
        Ok(ForkExec {
            program: p.clone(),
            sender,
            receiver,
        })
    }

    /// Awaits the next message on the control channel and, if an Ack
    /// message is received, returns the acked ID.
    async fn receive_control_channel_ack(
        &self,
        mut control_channel: BufStream<UnixStream>,
    ) -> Result<String> {
        let mut line = String::new();
        let count = control_channel.read_line(&mut line).await?;
        if count == 0 {
            // Preloader has closed the connection. We assume it's dead.
            debug!("read 0 bytes off the worker control channel, it's dead");
            return Err(WorkerDied.into());
        }
        let ack_msg: WorkerControlMessage = serde_json::from_str(&line)?;
        match ack_msg {
            WorkerControlMessage::Ack { id } => Ok(id),
        }
    }
}

#[async_trait]
impl ProcessControl for ForkExec {
    async fn initialize(&mut self) -> Result<()> {
        // No preparation necessary - we're ready to launch immediately.
        Ok(())
    }
    async fn spawn_process(&mut self) -> Result<String> {
        let id = self.generate_id();
        let mut cmdline = self.program.cmdline.iter();
        let mut kleinhirn_vars: HashMap<&str, String> = HashMap::new();
        kleinhirn_vars.insert(WORKER_ID_ENV, id.to_string());
        kleinhirn_vars.insert(NAME_ENV, self.program.cmdline.join(" "));
        let worker_control = if self.program.ack_workers {
            let (their_fd, control_channel) = worker_ack::worker_status_stream()?;
            kleinhirn_vars.insert(WORKER_CONTROL_CHANNEL_ENV, their_fd.to_string());
            // kleinhirn_vars.insert(WORKER_VERSION, ) // TODO: configure/pass in the version string.
            Some((their_fd, control_channel))
        } else {
            None
        };
        let child = Command::new(cmdline.next().expect("no commandline given").clone())
            .args(cmdline)
            .envs(self.program.env.iter())
            .envs(kleinhirn_vars)
            .current_dir(
                self.program
                    .cwd
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| current_dir().expect("No current working directory")),
            )
            .spawn()
            .context("Spawning a worker")?;

        self.sender
            .send(Action::Fork(id.to_string(), child.id()))
            .await?;
        if let Some((_, control_channel)) = worker_control {
            let acked_id = self
                .receive_control_channel_ack(control_channel)
                .await
                .context("Receiving an ack from the control channel")?;
            if acked_id != id {
                bail!(
                    "Received ack for ID {:?}, but expected {:?}",
                    &acked_id,
                    &id
                );
            }
            self.sender.send(Action::Ack(acked_id)).await?;
        } else {
            self.sender.send(Action::Ack(id.to_string())).await?;
        }
        Ok(id)
    }

    async fn next_message(&mut self) -> Result<Message> {
        match self.receiver.recv().await {
            Some(Action::Fork(id, pid)) => Ok(Message::Launched { id, pid }),
            Some(Action::Ack(id)) => Ok(Message::Ack { id }),
            None => {
                bail!("fork_exec control channel got closed for some reason?");
            }
        }
    }
}

/// The environment variable name used to pass the worker ID to a
/// subprocess in the fork/exec method. It is `$KLEINHIRN_WORKER_ID`.
pub const WORKER_ID_ENV: &str = "KLEINHIRN_WORKER_ID";

/// The environment variable name used to pass the worker's control
/// channel FD number. It is `$KLEINHIRN_CONTROL_FD`.
pub const WORKER_CONTROL_CHANNEL_ENV: &str = "KLEINHIRN_STATUS_FD";

/// The environment variable name used to pass the service name. It is
/// `$KLEINHIRN_NAME`.
pub const NAME_ENV: &str = "KLEINHIRN_NAME";

/// The environment variable name used to pass the version of the
/// service. It is `$KLEINHIRN_VERSION`.
#[allow(dead_code)]
pub const VERSION_ENV: &str = "KLEINHIRN_VERSION";
