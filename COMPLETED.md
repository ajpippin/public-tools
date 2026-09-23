# Completed

## Rust tool ports

- Ported `ollama-shelf` from the standalone Go binary to Rust.
- Ported `template` from the standalone Go binary to Rust.
- Ported `topic` from the standalone Go binary to Rust.
- Preserved the commands, help, configuration behavior, and tests for each tool.
- Preserved `template`'s 27 bundled templates.
- Preserved `topic` support for aliases, searching, sections, editing, Git updates,
  encrypted topics, and GPG recipients.

## Building and installation

- Configured release builds for all three Rust binaries.
- Set the default binary installation directory to `$HOME/.local/bin`.
- Set the bundled-template installation directory to
  `$HOME/.local/etc/templates.d`.
- Added per-tool hidden makefiles with build, test, clean, and install targets.
- Added a top-level hidden makefile that discovers Rust tools and can build,
  test, clean, or install all tools or one selected with `TOOL=name`.
- Made bundled-template installation non-destructive: locally modified files are
  retained and reported while missing templates continue to install.

## Configuration

- Standardized configuration files under `$HOME/.config/tools`:
  - `shelves.yaml`
  - `template.yaml`
  - `topic.yaml`
- Retained environment-variable and `--config` overrides.
- Added automatic creation of missing configuration files.
- Generated configuration files show every built-in setting as a commented-out
  default and do not overwrite existing files.
- Kept YAML as the common configuration format.

## Verification and repository updates

- Added unit and CLI integration tests for the Rust ports.
- Verified formatting, tests, Clippy with warnings denied, release builds, CLI
  help, and isolated installations.
- Committed and pushed the `ollama-shelf` Rust port.
- Committed and pushed the `template` and `topic` Rust ports, unified
  configuration, and top-level build orchestration.
- Committed and pushed the top-level install target and safe template installer.

## Continuous integration

- Split the top-level makefile into `lint` (rustfmt check and Clippy), `test`,
  `check`, and `coverage` (per-tool LCOV reports via `cargo-llvm-cov`).
- Added Bitbucket Pipelines: lint, test and coverage on every branch; Trivy
  scanning and SonarQube analysis with coverage and quality gate on `main`.
