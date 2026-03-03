all: build clippy check-fmt test

build:
  cargo build
  cargo build --features "defmt-log" --no-default-features

clippy:
  cargo clippy

fmt:
  cargo fmt --all

check-fmt:
  cargo fmt --all -- --check

test:
  cargo test
  cargo test --features "defmt-log" --no-default-features
