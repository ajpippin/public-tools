CARGO ?= cargo
BINDIR ?= $(HOME)/.local/bin
TEMPLATE_DIR ?= $(HOME)/.local/etc/templates.d
BINARY := template

.DEFAULT_GOAL := build

.PHONY: all build install install-templates test clean

all: build

build:
	$(CARGO) build --release

install: build install-templates
	install -d "$(BINDIR)"
	install -m 0755 "target/release/$(BINARY)" "$(BINDIR)/$(BINARY)"

install-templates:
	install -d "$(TEMPLATE_DIR)"
	@set -eu; \
	for source in template.d/*; do \
		name=$${source##*/}; \
		target="$(TEMPLATE_DIR)/$$name"; \
		if test ! -e "$$target"; then \
			install -m 0644 "$$source" "$$target"; \
		elif ! cmp -s "$$source" "$$target"; then \
			echo "warning: keeping modified template $$target" >&2; \
		fi; \
	done

test:
	$(CARGO) test
	$(CARGO) clippy --all-targets -- -D warnings

clean:
	$(CARGO) clean
