# syntax=docker/dockerfile:experimental

ARG RUST_VERSION=stable
FROM liuchong/rustup:$RUST_VERSION
MAINTAINER Andreas Fuchs <asf@boinkor.net>

ADD . /src
WORKDIR /src

RUN --mount=type=cache,target=/cache --mount=type=cache,target=/src/target --mount=type=cache,target=/src/xtask/target \
    ["env", "CARGO_HOME=/cache", "cargo", "xtask", "test"]
