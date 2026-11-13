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
- Moved CI from Bitbucket Pipelines to GitHub Actions
  (`.github/workflows/ci.yml`), following the titletool workflow. Trivy and
  SonarQube now run on every branch, runners are chosen by the `RUNS_ON`
  variable, and SonarQube is skipped when `SONAR_HOST_URL` is unset.

## Test coverage

- Expanded the unit and CLI integration tests for all three tools, raising
  coverage to 98.2% (merged in pull request #1):
  - `ollama-shelf`: 7 to 41 tests. Covers task routing, byte formatting,
    configuration loading and host selection, and `pull`, `gc`, `size`, and
    `bench` end to end with a fake `ollama` on `PATH` and a stub HTTP server.
  - `template`: 9 to 44 tests. Covers target planning, template lookup,
    overwrite/backup prompts, rendering and headers, configuration loading,
    and every subcommand.
  - `topic`: 9 to 52 tests. Covers topic lookup, listing, search, footers,
    configuration validation, and every command, including `edit` against a
    Git checkout with a bare origin, a fake `gpg`, and scripted editors.
- Isolated the CLI tests from the real `HOME`, so running them no longer creates
  configuration files under `$HOME/.config/tools`.
- Fixed bugs found while adding tests:
  - `template` panicked on long non-ASCII file names in `%%fileheader%%` and
    `%%header-dashes%%`.
  - `template show` reported unknown substitutions as `Some("name")`.
  - `topic` ignored `--config=FILE` when resolving aliases, so it loaded (and
    could create) the default configuration.
- Changed `topic list 00` to cover 0000-0099, matching other two-digit
  sections; `topic list 0` still covers 0000-0999.

## New tools

- Added `ytd`, a Rust wrapper around yt-dlp, and included it in the CI caches,
  the coverage check, and the SonarQube paths. `get` downloads with a configured
  directory, file name template, and default options, and `list` shows the
  videos in a playlist or channel.
- Moved the `ytd` configuration from YAML to `$XDG_CONFIG_HOME/etc/ytd.toml`, and
  added an `[env]` table for default environment values.
- Added `[aliases]` to `ytd`, with the same expansion rules as `tt`.
- Added `ytd init`, which prints a default configuration file to stdout. `ytd` no
  longer creates its configuration, and stops with its help when the file is
  missing.

- Changed `ytd list` to print JSON by default, with `--text` for a line per video.
- Fixed `ytd` rejecting bare video IDs that start with `-`.
- Made `ytd list` extract the playlist from a YouTube URL's `list=` value.
- Made `ytd get` download only the video from a video URL within a playlist.
- Added `ytd get --transcript`, which saves a video's captions as plain text.
- Made `ytd get --audio` save MP3 instead of the source format, such as Opus.
- Added `ytd get --type`, which takes `mp4`, `mp3`, `txt`, or `all` and saves
  any mix of the video, the audio, and the captions. Plain `ytd get` now saves
  the video as MP4.
- Made `ytd get --type mp4,mp3` download the video once and extract the MP3 from
  it with ffmpeg, set by the new `ffmpeg_binary` setting.
- Added `ytd get --video`, the same as `--type mp4`.

## Configuration lookup

- Made every tool look for its configuration in `$XDG_CONFIG_HOME/tools`,
  `~/.config/tools`, `$XDG_CONFIG_HOME`, then `~/.config`, using the first file
  that exists. `ytd` moved from `$XDG_CONFIG_HOME/etc/ytd.toml` to this order.

## Build system

- Renamed the top-level `.makefile` to `Makefile`, so `make` works without `-f`.
  The per-tool `.makefile` files are unchanged.
