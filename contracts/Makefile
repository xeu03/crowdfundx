# CrowdfundX contracts — build & test targets
#
#   make build   - compile all contracts to wasm32v1-none release
#   make test    - build wasm (required by cross-contract tests) + run tests
#   make check   - build + tests + clippy

.PHONY: build test check

build:
	cargo build --target wasm32v1-none --release

test: build
	cargo test

check: build
	cargo test
	cargo clippy --all-targets -- -D warnings
