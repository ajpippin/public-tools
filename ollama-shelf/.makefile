CARGO ?= cargo
BINDIR ?= $(HOME)/.bin/local
BINARY := ollama-shelf

.DEFAULT_GOAL := build

.PHONY: all build install test clean

all: build

build:
	$(CARGO) build --release

install: build
	install -d "$(BINDIR)"
	install -m 0755 "target/release/$(BINARY)" "$(BINDIR)/$(BINARY)"

test:
	$(CARGO) test
	$(CARGO) clippy --all-targets -- -D warnings

clean:
	$(CARGO) clean
