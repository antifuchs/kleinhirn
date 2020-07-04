# syntax=docker/dockerfile:experimental

ARG RUST_VERSION=stable
FROM rust:1.44
MAINTAINER Andreas Fuchs <asf@boinkor.net>

VOLUME /src

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
        apt-get update -qq && apt-get install -y autoconf bison build-essential libssl-dev libyaml-dev libreadline6-dev zlib1g-dev libncurses5-dev git procps lsof lldb strace htop lld-7 gdb # &&
        # cargo install sccache

RUN git clone https://github.com/rbenv/rbenv.git $HOME/.rbenv
RUN git clone https://github.com/rbenv/ruby-build.git $HOME/.rbenv/plugins/ruby-build
RUN bash -c 'export PATH="$HOME/.rbenv/bin:$PATH" ; eval $(rbenv init -) ; rbenv install 2.6.5 && rbenv global 2.6.5'
RUN echo 'export PATH="$HOME/.rbenv/bin:$PATH"' >> ~/.bashrc && \
        echo 'eval "$(rbenv init -)"' >> ~/.bashrc && \
        mkdir -p /target /.cargo && \
        echo '[build]\ntarget-dir = "/target"' > /.cargo/config
WORKDIR /src
