use super::{LogLevel, PreloaderSpecificMessage};
use crate::preloader::PreloaderMessage;
use slog::{self, b, record_static};
use std::collections::HashMap;

struct DynKV<'a>(&'a HashMap<String, String>);

impl<'a> slog::KV for DynKV<'a> {
    fn serialize(
        &self,
        _record: &slog::Record,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        for (k, v) in self.0.iter() {
            serializer.emit_str(k.to_string().into(), v)?;
        }
        Ok(())
    }
}

impl Into<slog::Level> for &LogLevel {
    fn into(self) -> slog::Level {
        match self {
            LogLevel::Debug => slog::Level::Debug,
            LogLevel::Info => slog::Level::Info,
        }
    }
}

/// Logs a message that came in from the preloader; if the message is
/// not a log message, returns Some(that message).
pub(super) fn translate_message(msg: PreloaderMessage) -> Option<PreloaderMessage> {
    if let PreloaderMessage::Preloader(PreloaderSpecificMessage::Log { level, msg, kv }) = msg {
        let rec_s = record_static!((&level).into(), "preloader");
        let mykv = DynKV(&kv);
        slog_scope::logger().log(&slog::Record::new(
            &rec_s,
            &format_args!("{}", msg),
            b!(mykv),
        ));
        return None;
    }
    Some(msg)
}
