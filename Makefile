CARGO ?= cargo
TOOLS := $(sort $(patsubst %/Cargo.toml,%,$(wildcard */Cargo.toml)))
SELECTED_TOOLS := $(if $(strip $(TOOL)),$(TOOL),$(TOOLS))
COVERAGE_DIR ?= coverage

.DEFAULT_GOAL := build

.PHONY: all build install lint test coverage check clean list help validate

all: build

help:
	@echo "Usage: make [build|install|lint|test|coverage|check|clean|list] [TOOL=name]"
	@echo
	@echo "Without TOOL, targets operate on every discovered Rust tool."
	@echo "coverage writes one LCOV report per tool to $(COVERAGE_DIR)/<tool>.lcov."

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

install: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "install $$tool"; \
		$(MAKE) -C "$$tool" -f .makefile install; \
	done

lint: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "lint   $$tool"; \
		$(CARGO) fmt --check --manifest-path "$$tool/Cargo.toml"; \
		$(CARGO) clippy --all-targets --manifest-path "$$tool/Cargo.toml" -- -D warnings; \
	done

test: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "test   $$tool"; \
		$(CARGO) test --manifest-path "$$tool/Cargo.toml"; \
	done

check: lint test

# Runs each tool's tests under instrumentation (replaces `test`). Source paths
# are made relative to the repository root so SonarQube can resolve them.
coverage: validate
	@set -eu; \
	mkdir -p "$(COVERAGE_DIR)"; \
	for tool in $(SELECTED_TOOLS); do \
		echo "cover  $$tool"; \
		$(CARGO) llvm-cov --manifest-path "$$tool/Cargo.toml" --lcov --output-path "$(COVERAGE_DIR)/$$tool.lcov"; \
		sed -i.bak "s|^SF:$(CURDIR)/|SF:|" "$(COVERAGE_DIR)/$$tool.lcov"; \
		rm -f "$(COVERAGE_DIR)/$$tool.lcov.bak"; \
		$(CARGO) llvm-cov report --manifest-path "$$tool/Cargo.toml" --summary-only; \
	done

clean: validate
	@set -eu; \
	for tool in $(SELECTED_TOOLS); do \
		echo "clean  $$tool"; \
		$(CARGO) clean --manifest-path "$$tool/Cargo.toml"; \
	done; \
	rm -rf "$(COVERAGE_DIR)"
