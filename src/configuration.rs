use futures::stream::pending;
use serde::Deserialize;
use std::collections::HashMap;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    stream::Stream,
    time::{interval, Instant},
};

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Config {
    #[serde(default)]
    pub health_check: HealthConfig,

    #[serde(default)]
    pub log: LoggingConfig,
    pub supervisor: SupervisorConfig,
    pub worker: WorkerConfig,

    #[serde(skip)]
    pub base_dir: PathBuf,
}

impl Config {
    #[allow(dead_code)] // TODO: use this more consistently
    pub(crate) fn canonical_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.base_dir.join(path)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct HealthConfig {
    #[serde(default)]
    pub listen_addr: Option<SocketAddr>,

    #[serde(default = "default_health_endpoint")]
    pub endpoint: String,
}

fn default_health_endpoint() -> String {
    "/healthz".to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LoggingConfig {
    #[serde(default)]
    pub format: LogFormat,
    #[serde(default)]
    pub output: LogOutput,
    #[serde(default)]
    pub level: LogLevel,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            format: LogFormat::Logfmt { print_prefix: true },
            output: Default::default(),
            level: Default::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogLevel {
    Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

impl Into<slog::Level> for LogLevel {
    fn into(self) -> slog::Level {
        use slog::Level::*;
        match self {
            Self::Critical => Critical,
            Self::Error => Error,
            Self::Warning => Warning,
            Self::Info => Info,
            Self::Debug => Debug,
            Self::Trace => Trace,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "format")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LogFormat {
    Json,
    Logfmt { print_prefix: bool },
}

impl Default for LogFormat {
    fn default() -> Self {
        LogFormat::Logfmt { print_prefix: true }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LogOutput {
    Stderr,
    Stdout,
}

impl Default for LogOutput {
    fn default() -> Self {
        LogOutput::Stderr
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SupervisorConfig {
    /// Name of the supervised service. Determines logging fields and defaults for the socket
    /// name.
    pub name: String,

    /// UNIX domain socket on which to listen for commands. Defaults to "/tmp/kleinhirn-<name>.sock".
    /// Also creates a lock file next to the socket, in order to prevent multiple uncontrollable
    /// copies from being created.
    pub socket: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct WorkerConfig {
    /// Number of workers to spawn. Default: 1
    #[serde(default = "default_count")]
    pub count: usize,

    #[serde(flatten)]
    pub kind: WorkerKind,

    #[serde(default)]
    #[serde(with = "humantime_serde")]
    pub ack_timeout: Option<Duration>,
}

impl WorkerConfig {
    pub fn ack_ticker(&self) -> Box<dyn Stream<Item = Instant> + Unpin> {
        if let Some(timeout) = self.ack_timeout {
            Box::new(interval(timeout / 2))
        } else {
            Box::new(pending())
        }
    }
}

fn default_count() -> usize {
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Program {
    pub cmdline: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,

    #[serde(default)]
    pub ack_workers: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ruby {
    /// The pathname identifying the Gemfile for the bundled ruby
    /// application.
    pub gemfile: PathBuf,

    /// A ruby file that can be `load`ed.
    pub load: PathBuf,

    /// A ruby expression that each worker runs in order to start.
    pub start_expression: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum WorkerKind {
    /// Supervise a program that gets forked & exec'ed [`WorkerConfig.count`] times. This does
    /// not support any variable substitution
    /// or shell expansion.
    Program(Program),

    /// Supervise a bundled ruby program that can be preloaded. That
    /// bundle is expected to include the `kleinhirn_loader` gem,
    /// which is then launched via the following expression:
    ///
    /// ```sh
    /// bundle exec --gemfile=<gemfile_path> --keep-file-descriptors \
    ///             kleinhirn_loader -- <load> <start_expression>
    /// ```
    #[cfg(target_os = "linux")]
    Ruby(Ruby),
}
