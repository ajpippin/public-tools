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
default is `$HOME/.local/bin`; override it through the environment or on the
command line:

```sh
BINDIR=/some/other/bin make -f .makefile install
```

Without an override, the configuration is the first of these that exists:

1. `$XDG_CONFIG_HOME/tools/shelves.yaml`
2. `~/.config/tools/shelves.yaml`
3. `$XDG_CONFIG_HOME/shelves.yaml`
4. `~/.config/shelves.yaml`

`XDG_CONFIG_HOME` is skipped when it is unset, empty, or relative.
When none exists, the first is created. `OLLAMA_SHELF_CONFIG` overrides the
lookup, and `--config` takes precedence over both. `OLLAMA_HOST_URL` and
`OLLAMA_MODELS` override their corresponding YAML values.

If the file is missing, `ollama-shelf` creates it automatically with every
available setting shown as a commented-out default. Existing files are never
rewritten during startup.

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
