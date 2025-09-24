# Simple Makefile for snapper-tui

.PHONY: all help build run sudo-run release clean fmt clippy test

all: build ## Build (alias)

help: ## Show this help message
	@printf "\nSnapper TUI — Make targets\n\n"
	@awk -F ':|##' '/^[a-zA-Z0-9_.-]+:.*##/ { \
	  name=$$1; desc=$$3; \
	  gsub(/^[ \t]+|[ \t]+$$/, "", name); \
	  gsub(/^[ \t]+|[ \t]+$$/, "", desc); \
	  printf "  \033[36m%-14s\033[0m %s\n", name, desc \
	}' $(MAKEFILE_LIST)

build: ## Build debug binary
	cargo build

run: ## Run in debug mode
	cargo run

sudo-run: ## Run with sudo (preserve env)
	sudo -E cargo run

release: ## Build release binary
	cargo build --release

clean: ## Remove build artifacts
	cargo clean

fmt: ## Format all code with rustfmt
	cargo fmt --all

clippy: ## Lint with clippy (deny warnings)
	cargo clippy --all-targets -- -D warnings

test: ## Run tests (all crates)
	cargo test --all
