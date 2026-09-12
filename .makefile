CARGO ?= cargo
TOOLS := $(sort $(patsubst %/Cargo.toml,%,$(wildcard */Cargo.toml)))
SELECTED_TOOLS := $(if $(strip $(TOOL)),$(TOOL),$(TOOLS))

.DEFAULT_GOAL := build

.PHONY: all build test clean list help validate

all: build

help:
	@echo "Usage: make -f .makefile [build|test|clean|list] [TOOL=name]"
	@echo
	@echo "Without TOOL, build, test, and clean operate on every discovered Rust tool."

list:
	@for tool in $(TOOLS); do echo "$$tool"; done

validate:
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		test -f "$$tool/Cargo.toml" || { echo "unknown Rust tool: $$tool" >&2; exit 1; }; \
	done

build: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "build  $$tool"; \
		$(CARGO) build --release --manifest-path "$$tool/Cargo.toml"; \
	done

test: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "test   $$tool"; \
		$(CARGO) test --manifest-path "$$tool/Cargo.toml"; \
		$(CARGO) clippy --all-targets --manifest-path "$$tool/Cargo.toml" -- -D warnings; \
	done

clean: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "clean  $$tool"; \
		$(CARGO) clean --manifest-path "$$tool/Cargo.toml"; \
	done
