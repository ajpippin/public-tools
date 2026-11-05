# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository layout

Four independent Rust CLI crates: `ollama-shelf`, `template`, `topic`, and `ytd`. There is no Cargo workspace and no root `Cargo.toml`, so `cargo` commands must run inside a crate directory or use `--manifest-path`. Each crate has its own `Cargo.lock` and `target/`.

Each tool's README is its full command and configuration reference. `docs/` holds the original shell and C++ sources and the prompt used for the first port; nothing builds from it.

## Commands

The makefile is hidden, so `-f .makefile` is required. Omit `TOOL=` to run against every tool.

```sh
make -f .makefile build [TOOL=topic]
make -f .makefile lint [TOOL=topic]      # rustfmt --check, then Clippy with -D warnings
make -f .makefile test [TOOL=topic]
make -f .makefile check [TOOL=topic]     # lint, then test
make -f .makefile coverage [TOOL=topic]  # needs cargo-llvm-cov and llvm-tools-preview
```

Run a single test, or only the integration tests:

```sh
cargo test --manifest-path topic/Cargo.toml find_rejects_ambiguous_topics
cargo test --manifest-path topic/Cargo.toml --test cli
```

Each tool also has its own `.makefile`. Its `test` target runs Clippy too, unlike the top-level `test` target.

## Architecture

All the tools share a shape: `main.rs` calls `cli::run()` and prints errors with `{error:#}`, and `cli.rs` holds the clap definitions and command dispatch. `config.rs` handles the configuration, which is YAML except in `ytd`, where it is TOML. The domain logic is in `template/src/engine.rs` and `topic/src/store.rs`; `ollama-shelf` and `ytd` keep their logic in `cli.rs` and `config.rs`. The tools share no code, so the same pattern is duplicated across crates, and a change to shared behavior usually has to be made in each one.

### Configuration

- Lookup order is `--config`, then the tool's env var (`OLLAMA_SHELF_CONFIG`, `TEMPLATE_CONFIG`, `TOPIC_CONFIG`, `YTD_CONFIG`), then `~/.config/tools/<shelves|template|topic>.yaml`, or `$XDG_CONFIG_HOME/etc/ytd.toml` for `ytd`.
- `config::ensure` creates a missing file on any normal run, and an existing file is never rewritten. `ytd` is the exception: it has no `ensure`, stops with an error and its help when the file is missing, and `ytd init` prints the starter text to stdout.
- The generated file shows every default as a commented-out value. New settings must be added to that starter text as well as the structs; unit tests in `config.rs` check that the starter config stays in step with the defaults.

### Tool-specific behavior

- `topic`: aliases are resolved by `resolve_aliases`, which rewrites `argv` before clap parses it. This means configuration is loaded ahead of argument parsing, and `config_argument` has to find `--config` by hand in both the `--config FILE` and `--config=FILE` forms.
- `topic`: `edit` runs the editor, updates the footer, and commits and pushes through `git`. Encrypted topics go through the configured `gpg` binary.
- `template`: `template/template.d/` holds the bundled templates, which are data files rather than code. `install` copies them to `~/.local/etc/templates.d` and keeps any local file whose contents differ.
- `ollama-shelf`: shells out to the `ollama` binary and calls its HTTP API for benchmarks. Shelves are selected per host from the configuration file.
- `ytd`: `get` and `list` each build a `yt-dlp` command from the configuration and the command line. URLs follow a `--` so that they can never be read as options. Like `topic`, it loads configuration ahead of argument parsing, because `expand_aliases` rewrites `argv` before clap parses it. `run` tries clap once first, so that `init`, `--help`, and `--version` work without a usable configuration. The `[env]` table in `ytd.toml` sets default environment variables for `yt-dlp`, used only when a variable is not already set. CLI test fixtures for `ytd` must also remove `XDG_CONFIG_HOME`.
- `template` and `topic` use a `build.rs` to embed the Git commit and Git version for `version.rs`, through tool-prefixed env vars. `ytd` uses a `build.rs` to embed the Git tag (or `dev`) and the 7-character commit SHA for `--version`. `ollama-shelf` has neither file.

## Testing conventions

- Unit tests sit in `#[cfg(test)]` modules next to the code. CLI integration tests are in each crate's `tests/cli.rs`, using `assert_cmd`.
- CLI tests must not touch the real `HOME`. Fixtures set `HOME` to a temp directory and remove the tool's env-var overrides; follow the existing fixture in that crate's `tests/cli.rs` when adding tests.
- External programs are faked rather than required: a fake `ollama` on `PATH` and a stub HTTP server for `ollama-shelf`, a fake `gpg`, scripted editors, and a Git checkout with a bare origin for `topic`, and a fake `yt-dlp` that records its arguments for `ytd`.

## CI and adding a tool

`bitbucket-pipelines.yml` runs lint and coverage on every branch, and adds Trivy and SonarQube on `main`. The makefile discovers tools automatically through `*/Cargo.toml`, but CI does not: a new tool must also be added to the cache definitions and the `for tool in ...` check in `bitbucket-pipelines.yml`, and to the source, test, and LCOV paths in `sonar-project.properties`.

## Project records

`COMPLETED.md` logs finished work and `TASKS.md` tracks outstanding tasks. Update them when completing notable work. `topic/TODO.md` lists ideas for `topic`.
