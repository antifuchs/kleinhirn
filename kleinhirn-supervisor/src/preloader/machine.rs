use super::{LogLevel, PreloaderMessage};
use machine::*;
use slog::{self, b, record_static};
use slog_scope::{debug, error, warn};
use std::collections::HashMap;

machine! {
    #[derive(Clone, PartialEq, Debug)]
    pub enum PreloaderState {
        Starting,
        Loading,
        Ready,
        Failed,
    }
}

transitions!(PreloaderState, [
    (Starting, PreloaderMessage) => [Starting, Loading, Error],
    (Loading, PreloaderMessage) => [Loading, Ready, Error]
]);

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

fn maybe_log(msg: &PreloaderMessage) -> bool {
    if let PreloaderMessage::Log { level, msg, kv } = msg {
        let rec_s = record_static!(level.into(), "preloader");
        let mykv = DynKV(kv);
        slog_scope::logger().log(&slog::Record::new(
            &rec_s,
            &format_args!("{}", msg),
            b!(mykv),
        ));
        return true;
    }
    return false;
}

impl Starting {
    pub fn on_preloader_message(self, msg: PreloaderMessage) -> PreloaderState {
        if maybe_log(&msg) {
            return PreloaderState::starting();
        }
        match msg {
            PreloaderMessage::Loading { file } => {
                debug!("loading"; "file" => ?file);
                PreloaderState::loading()
            }
            _ => PreloaderState::failed(),
        }
    }
}

impl Loading {
    pub fn on_preloader_message(self, msg: PreloaderMessage) -> PreloaderState {
        if maybe_log(&msg) {
            return PreloaderState::loading();
        }
        match msg {
            PreloaderMessage::Loading { .. } => PreloaderState::loading(),
            PreloaderMessage::Ready => {
                debug!("Preloader is ready");
                PreloaderState::ready()
            }
            PreloaderMessage::Error { message, error } => {
                error!("Communication error with the preloader. This is a bug."; "message" => %message, "error" => ?error);
                PreloaderState::failed()
            }
            PreloaderMessage::Failed { id, message } => {
                warn!("Command failed"; "id" => id, "message" => %message);
                PreloaderState::failed()
            }
            _ => PreloaderState::loading(),
        }
    }
}
