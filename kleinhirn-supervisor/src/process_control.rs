use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// A message that updates the supervisor on the state of a child
/// process.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Launched { id: String, pid: u32 },
    Ack { id: String },
}

#[async_trait]
pub trait ProcessControl {
    /// Returns success when the process controller is
    /// initialized. This is a no-op on regular programs, but a
    /// preloader will resolve here when the code is loaded.
    async fn initialize(&mut self) -> Result<()>;

    /// Generates a child ID, spawns the process (probably forking, or
    /// double-forking), and returns that ID on success.
    async fn spawn_process(&mut self) -> Result<String>;

    /// Returns the next update of the process control scheme.
    ///
    /// If this returns an error, the process control invariants have
    /// broken down.
    async fn next_message(&mut self) -> Result<Message>;

    /// Generates a UUID-based ID string.
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}
