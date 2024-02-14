FROM rust:1.75

RUN apt-get update && apt-get install -y \
  libclang-dev \
  libudev-dev
