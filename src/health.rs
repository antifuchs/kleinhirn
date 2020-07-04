use crate::configuration;
use anyhow::{Context, Result};
use async_dup::Arc;
use futures::{future::pending, AsyncRead, AsyncWrite};
use http::{response::Response, Method, StatusCode};
use slog_scope::warn;
use smol::{Async, Task};
use std::{
    convert::Infallible,
    net::{TcpListener, TcpStream},
};
use tophat::{
    server::{
        accept,
        router::{Router, RouterRequestExt},
        Glitch, ResponseWriter, ResponseWritten,
    },
    Body, Request,
};

async fn serve_health<T: HealthIndicator + 'static, W>(
    req: Request,
    mut resp_wtr: ResponseWriter<W>,
) -> Result<ResponseWritten, Glitch>
where
    W: AsyncRead + AsyncWrite + Clone + Send + Sync + Unpin + 'static,
{
    if let Some(indicator) = req.data::<T>() {
        *resp_wtr.response_mut() = indicator.health_check().response();
    } else {
        resp_wtr.set_status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    resp_wtr.send().await
}

pub(crate) async fn healthcheck_server<T: HealthIndicator + Clone + 'static>(
    config: configuration::HealthConfig,
    check: T,
) -> Result<Infallible> {
    if let Some(addr) = config.listen_addr {
        let router = Router::build()
            .data(check)
            .at(
                Method::GET,
                &config.endpoint,
                serve_health::<T, Arc<Async<TcpStream>>>,
            )
            .finish();

        let listener = Async::<TcpListener>::bind(addr)
            .context("Couldn't listen on HTTP healthcheck address")?;

        loop {
            let (stream, _) = listener.accept().await?;
            let stream = Arc::new(stream);
            let router = router.clone();

            let task = Task::spawn(async move {
                let serve = accept(stream, |req, resp_wtr| async {
                    router.route(req, resp_wtr).await
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

/// State of a health check result.
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
            .unwrap() // a technicality: The above can't fail.
    }
}

pub(crate) trait HealthIndicator: Send + Sync + Unpin {
    fn health_check(&self) -> State;
}
