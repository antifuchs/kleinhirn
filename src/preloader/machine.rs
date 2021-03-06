use super::{PreloaderMessage, PreloaderSpecificMessage};
use machine::*;
use slog_scope::{debug, error, warn};

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

impl Starting {
    pub fn on_preloader_message(self, msg: PreloaderMessage) -> PreloaderState {
        use PreloaderMessage::*;
        use PreloaderSpecificMessage::*;

        match msg {
            Preloader(Loading { file }) => {
                debug!("loading"; "file" => ?file);
                PreloaderState::loading()
            }
            _ => PreloaderState::failed(),
        }
    }
}

impl Loading {
    pub fn on_preloader_message(self, msg: PreloaderMessage) -> PreloaderState {
        use PreloaderMessage::*;
        use PreloaderSpecificMessage::*;

        match msg {
            Preloader(Loading { .. }) => PreloaderState::loading(),
            Preloader(Ready) => {
                debug!("Preloader is ready");
                PreloaderState::ready()
            }
            Preloader(Error { message, error }) => {
                error!("Communication error with the preloader. This is a bug."; "message" => %message, "error" => ?error);
                PreloaderState::failed()
            }
            Preloader(Failed { id, message }) => {
                warn!("Command failed"; "id" => id, "message" => %message);
                PreloaderState::failed()
            }
            _ => PreloaderState::loading(),
        }
    }
}
