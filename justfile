default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace

check:
    cargo check --workspace --all-targets

verify: fmt-check lint test check

build:
    cargo build --workspace --release

dist-plan:
    cargo dist plan
