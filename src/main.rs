use anyhow::Result;
use slog::{o, Drain, Logger};
use slog_scope::info;

use tokio::runtime::Runtime;

pub mod supervisor;

fn create_logger() -> Logger {
    // This should do for now, but
    // TODO: Use json logging
    let decorator = slog_term::PlainSyncDecorator::new(std::io::stderr());
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    Logger::root(drain, o!("logger" => "kleinhirn"))
}

fn main() -> Result<()> {
    let mut rt = Runtime::new()?;

    let log = create_logger();
    let _guard = slog_scope::set_global_logger(log);

    info!("startup");
    rt.block_on(async {
        supervisor::run().await?;

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
