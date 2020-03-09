# syntax=docker/dockerfile:experimental

ARG RUST_VERSION=stable
FROM liuchong/rustup:$RUST_VERSION
MAINTAINER Andreas Fuchs <asf@boinkor.net>

VOLUME /src

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
        apt-get update -qq && apt-get install -y autoconf bison build-essential libssl-dev libyaml-dev libreadline6-dev zlib1g-dev libncurses5-dev git procps

RUN git clone https://github.com/rbenv/rbenv.git ~/.rbenv
RUN git clone https://github.com/rbenv/ruby-build.git ~/.rbenv/plugins/ruby-build
RUN echo 'export PATH="$HOME/.rbenv/bin:$PATH"' >> ~/.bashrc
RUN bash -c 'export PATH="$HOME/.rbenv/bin:$PATH" ; eval $(rbenv init -) ; rbenv install 2.6.5'
RUN echo 'eval "$(rbenv init -)"' >> ~/.bashrc
RUN bash -c 'export PATH="$HOME/.rbenv/bin:$PATH" ; eval $(rbenv init -) ; rbenv global 2.6.5'
WORKDIR /src
