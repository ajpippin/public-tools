# template

A Rust edition of the historical [`../docs/template.cc`](../docs/template.cc)
file generator.

```text
template [--template name] [--force|--backup] <target>...
template list
template show [substitution]
template init
template version
```

Templates are selected by `--template`, by an exact template matching the
target filename, or by the target extension. Compound `.ch` and `.cchh`
targets create `.c`/`.h` and `.cc`/`.hh` pairs respectively.

Install the release binary into `$BINDIR` (default `$HOME/.local/bin`) and the
maintained templates into `$TEMPLATE_DIR` (default
`$HOME/.local/etc/templates.d`):

```sh
make -f .makefile install
```

To install only the bundled templates:

```sh
make -f .makefile install-templates
```

Template installation never overwrites an existing file with different
contents. Modified templates are retained and reported as warnings while all
other missing templates continue to install.

Without an override, the configuration is the first of these that exists:

1. `$XDG_CONFIG_HOME/tools/template.yaml`
2. `~/.config/tools/template.yaml`
3. `$XDG_CONFIG_HOME/template.yaml`
4. `~/.config/template.yaml`

`XDG_CONFIG_HOME` is skipped when it is unset, empty, or relative.
When none exists, the first is used. `TEMPLATE_CONFIG` and `--config` override
the lookup. Run
`template init` to create a starter configuration; `TEMPLATE_DIR` remains
supported as an additional path list when no configuration file exists.
The file is also created automatically when missing, with every setting shown
as a commented-out default; existing files are not rewritten during startup.

Existing targets prompt before replacement. Use `--force` to overwrite or
`--backup` to preserve the old target with a trailing `~`.
