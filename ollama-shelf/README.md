# ollama-shelf

A Rust command-line tool for managing a curated Ollama model shelf selected by
the current machine's hostname. It is a Rust port of the standalone Go
`ollama-shelf` utility.

## Build and install

```sh
make -f .makefile install
ollama-shelf init
```

`install` builds the release binary and copies it to `$BINDIR`. The current
default is `$HOME/.bin/local`; override it through the environment or on the
command line:

```sh
BINDIR=/some/other/bin make -f .makefile install
```

The default configuration is `ollama/shelves.yaml` beneath the operating
system's application-config directory. `OLLAMA_SHELF_CONFIG` overrides that
path, and `--config` takes precedence over both. `OLLAMA_HOST_URL` and
`OLLAMA_MODELS` override their corresponding YAML values.

## Commands

```text
ollama-shelf list [shelf|all]
ollama-shelf init
ollama-shelf pull [shelf|all]
ollama-shelf size
ollama-shelf gc [--yes]
ollama-shelf bench <model|shelf|all>
ollama-shelf which <task description>
```

Host selection tries the exact hostname, its short name, then a `default` or
`*` profile. The pseudo-name `all` means every unique model across all shelves.
