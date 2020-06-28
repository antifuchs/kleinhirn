use anyhow::{Error, Result};
use futures::prelude::*;
use hyper::{server::Builder, Server};
use smol::{Async, Task};
use std::{
    io,
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    pin::Pin,
    task::{Context, Poll},
};

/// Spawns futures.
#[derive(Clone)]
pub(super) struct SmolExecutor;

impl<F: Future + Send + 'static> hyper::rt::Executor<F> for SmolExecutor {
    fn execute(&self, fut: F) {
        Task::spawn(async { drop(fut.await) }).detach();
    }
}

/// Listens for incoming connections.
pub(super) struct SmolListener {
    listener: Async<TcpListener>,
}

impl SmolListener {
    fn new(listener: Async<TcpListener>) -> Self {
        Self { listener }
    }
}

impl hyper::server::accept::Accept for SmolListener {
    type Conn = SmolStream;
    type Error = Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let poll = Pin::new(&mut self.listener.incoming()).poll_next(cx);
        let stream = futures::ready!(poll).unwrap()?;
        Poll::Ready(Some(Ok(SmolStream(stream))))
    }
}

/// A TCP connection handled by smol.
#[derive(Debug)]
pub(super) struct SmolStream(Async<TcpStream>);

impl hyper::client::connect::Connection for SmolStream {
    fn connected(&self) -> hyper::client::connect::Connected {
        hyper::client::connect::Connected::new()
    }
}

impl tokio::io::AsyncRead for SmolStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for SmolStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.0.get_ref().shutdown(Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}

pub(super) fn server(listen_addr: &SocketAddr) -> Result<Builder<SmolListener, SmolExecutor>> {
    let listener = Async::<TcpListener>::bind(listen_addr)?;
    Ok(Server::builder(SmolListener::new(listener)).executor(SmolExecutor))
}
