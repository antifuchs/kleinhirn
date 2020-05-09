use crate::configuration;
use crate::process_control::{Message, ProcessControl};
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::env::current_dir;
use std::process::Command;
use tokio::sync::mpsc::{channel, Receiver, Sender};

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
        let child = Command::new(cmdline.next().expect("no commandline given").clone())
            .args(cmdline)
            .envs(self.program.env.iter())
            .current_dir(
                self.program
                    .cwd
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| current_dir().expect("No current working directory")),
            )
            .spawn()?;

        // TODO: implement acking via an FD.
        self.sender
            .send(Action::Fork(id.to_string(), child.id()))
            .await?;
        self.sender.send(Action::Ack(id.to_string())).await?;
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
