FROM rust:1.75

RUN apt-get update && apt-get install -y \
  zstd \
  libclang-dev \
  libudev-dev \
  libiio-dev \
  squashfs-tools
