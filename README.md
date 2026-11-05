# tools

Small standalone command-line utilities implemented in Rust.

## Included tools

### `ollama-shelf`

Manages host-specific collections of Ollama models. It can list and pull
curated models, report storage use, remove models outside the configured shelf,
benchmark models, and select a model for a described task.

### `template`

Creates files from reusable templates. It supports filename- and
extension-based template selection, substitutions, compound C/C++ targets, and
safe overwrite or backup handling. The repository includes 27 bundled
templates.

### `topic`

Browses and maintains a numbered personal topic library. It supports listing,
searching, sections, configurable command aliases, editing with canonical
timestamps, Git updates, and optional GPG-encrypted topics.

### `ytd`

Wraps yt-dlp with two commands: `get` downloads videos or audio using a
configured directory, file name template, and default options, and `list` shows
the videos in a playlist or channel without downloading them.

Each tool has its own README with its complete command and configuration
reference.

## Build and test

The top-level hidden makefile automatically discovers Rust tools in this
directory.

```sh
# Build, lint, or test every tool
make -f .makefile build
make -f .makefile lint      # rustfmt --check and Clippy with warnings denied
make -f .makefile test
make -f .makefile check     # lint, then test

# Run tests under cargo-llvm-cov and write coverage/<tool>.lcov
make -f .makefile coverage

# Operate on one tool
make -f .makefile build TOOL=topic
make -f .makefile test TOOL=template

# List discovered tools and makefile usage
make -f .makefile list
make -f .makefile help
```

Release builds use Cargo's standard optimized release profile.

`coverage` requires `cargo-llvm-cov` and the `llvm-tools-preview` rustup
component.

## Continuous integration

`bitbucket-pipelines.yml` runs on the self-hosted runner:

- **Every branch:** lint, then tests with coverage.
- **`main`:** additionally runs a Trivy filesystem scan (vulnerabilities,
  secrets, and misconfigurations; fails on fixable HIGH or CRITICAL findings)
  and a SonarQube analysis that imports the LCOV coverage and waits for the
  quality gate.

SonarQube settings are in `sonar-project.properties` (project key
`PublicTools`). The pipeline needs the `SONAR_HOST_URL` and secured
`SONAR_TOKEN` repository variables.

## Install

Install every binary:

```sh
make -f .makefile install
```

Install one binary:

```sh
make -f .makefile install TOOL=ollama-shelf
```

`BINDIR` defaults to `$HOME/.local/bin` and can be overridden:

```sh
make -f .makefile install BINDIR=/custom/bin
```

Installing `template` also installs its bundled templates under
`$HOME/.local/etc/templates.d`. Existing templates with different contents are
preserved and reported as warnings; installation continues for other missing
templates.

## Configuration

The tools keep their YAML configuration files together:

```text
~/.config/tools/
├── shelves.yaml
├── template.yaml
├── topic.yaml
└── ytd.yaml
```

Each binary automatically creates its missing configuration file with all
built-in defaults shown as commented-out values. Existing files are never
rewritten during startup. Each tool also supports its environment-variable and
`--config` overrides.

## Clean

```sh
# Remove build artifacts for every tool
make -f .makefile clean

# Remove build artifacts for one tool
make -f .makefile clean TOOL=topic
```

See [COMPLETED.md](COMPLETED.md) for the conversion and setup work already
completed. [TASKS.md](TASKS.md) records the current outstanding-task status.
