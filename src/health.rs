use crate::configuration;
use futures::future::pending;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Response, Server, StatusCode,
};
use slog_scope::info;
use std::{convert::Infallible, sync::Arc};

pub(crate) async fn healthcheck_server(
    config: configuration::HealthConfig,
    indicator: impl HealthIndicator + Clone + Send + Sync + 'static,
) -> hyper::Result<()> {
    if let Some(addr) = config.listen_addr {
        let endpoint = Arc::new(config.endpoint.clone());
        let svc = make_service_fn(move |_conn| {
            let indicator = indicator.clone();
            let endpoint = endpoint.clone();
            async move {
                // service_fn converts our function into a `Service`
                Ok::<_, Infallible>(service_fn(move |req| {
                    let response = indicator.health_check().response();
                    let endpoint = endpoint.clone();
                    async move {
                        match (req.method(), req.uri().path()) {
                            (&Method::GET, path) if path == *endpoint => response,
                            _ => Ok(Response::builder()
                                .status(StatusCode::NOT_FOUND)
                                .body(Body::from("not found\n"))
                                .unwrap()),
                        }
                    }
                }))
            }
        });
        let server = Server::bind(&addr).serve(svc);
        info!("health probe service listening"; "addr" => ?&addr, "endpoint" => config.endpoint);
        server.await
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
    fn response(&self) -> Result<Response<Body>, hyper::http::Error> {
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
    }
}

pub(crate) trait HealthIndicator {
    fn health_check(&self) -> State;
}
