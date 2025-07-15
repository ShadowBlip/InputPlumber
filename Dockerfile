FROM rust:1.84

RUN dpkg --add-architecture arm64
RUN apt-get update && apt-get install -y \
  zstd \
  libclang-dev \
  libudev-dev \
  libiio-dev \
  squashfs-tools

RUN apt-get install -y \
  g++-aarch64-linux-gnu \
  libc6-dev-arm64-cross \
  libudev-dev:arm64 \
  libiio-dev:arm64

RUN rustup target add aarch64-unknown-linux-gnu
RUN rustup toolchain install stable-aarch64-unknown-linux-gnu
RUN rustup component add clippy

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
ENV CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
