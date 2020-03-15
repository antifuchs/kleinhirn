use hyper::{
    service::{make_service_fn, service_fn},
    Body, Response, Server, StatusCode,
};

use std::convert::Infallible;

pub(crate) async fn healthcheck_server(
    indicator: impl HealthIndicator + Clone + Send + Sync + 'static,
) -> hyper::Result<()> {
    let addr = ([127, 0, 0, 1], 3000).into();
    let svc = make_service_fn(move |_conn| {
        let indicator = indicator.clone();
        async move {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(service_fn(move |_req| {
                let response = indicator.health_check().response();
                async move { response }
            }))
        }
    });
    let server = Server::bind(&addr).serve(svc);
    server.await
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
