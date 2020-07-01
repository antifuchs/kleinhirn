use crate::configuration;
use anyhow::{Context, Result};
use async_dup::Arc;
use futures::future::pending;
use http::{response::Response, Method, StatusCode};
use slog_scope::warn;
use smol::{Async, Task};
use std::net::TcpListener;
use tophat::{server::accept, Body};

pub(crate) async fn healthcheck_server(
    config: configuration::HealthConfig,
    indicator: impl HealthIndicator + Clone + Send + Sync + 'static,
) -> Result<()> {
    if let Some(addr) = config.listen_addr {
        let endpoint = Arc::new(config.endpoint.clone());
        let listener = Async::<TcpListener>::bind(addr)
            .context("Couldn't listen on HTTP healthcheck address")?;

        loop {
            let (stream, _) = listener.accept().await?;
            let indicator = indicator.clone();
            let stream = Arc::new(stream);
            let endpoint = endpoint.clone();
            let task = Task::spawn(async move {
                let indicator = indicator.clone();
                let endpoint = endpoint.clone();
                let serve = accept(stream, |req, mut resp_wtr| async {
                    let req = Arc::new(req);
                    match (req.method(), req.uri().path()) {
                        (&Method::GET, path) if path == *endpoint => {
                            *resp_wtr.response_mut() = indicator.health_check().response();
                        }
                        _ => {
                            resp_wtr.set_status(StatusCode::NOT_FOUND);
                        }
                    }
                    resp_wtr.send().await
                })
                .await;

                if let Err(err) = serve {
                    warn!("Error serving healthcheck request"; "err" => ?err);
                }
            });

            task.detach();
        }
    } else {
        pending().await
    }
}

#[derive(Debug)]
pub(crate) enum State {
    /// Everything is ok with this indicator
    Healthy,

    /// Something's unhealthy
    Unhealthy(Box<dyn std::error::Error>),
}

impl State {
    fn response(&self) -> Response<Body> {
        use State::*;
        Response::builder()
            .status(match self {
                Healthy => StatusCode::OK,
                Unhealthy(_) => StatusCode::EXPECTATION_FAILED,
            })
            .body(match self {
                Healthy => Body::from("ok\n"),
                Unhealthy(e) => Body::from(format!("unhealthy: {:?}\n", e)),
            })
            .unwrap_or_else(|_e| {
                let mut res = Response::new(Body::from(
                    "Failed to create response. This is a kleinhirn bug.",
                ));
                *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                res
            })
    }
}

pub(crate) trait HealthIndicator {
    fn health_check(&self) -> State;
}
