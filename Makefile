# Simple Makefile for snapper-tui

.PHONY: all build run sudo-run release clean fmt clippy test

all: build

build:
	cargo build

run:
	cargo run

sudo-run:
	sudo -E cargo run

release:
	cargo build --release

clean:
	cargo clean

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --all
