# topic

A Rust personal topic-library tool. Topics are four-digit `.txt` or `.md`
files, optionally encrypted with GPG. Configuration defaults to
`~/.config/tools/topic.yaml`; `TOPIC_CONFIG`, `--config`, and `--dir` provide
overrides.

When this file is missing, `topic` creates it automatically with every setting
shown as a commented-out default. Existing files are never rewritten during
startup.

```text
topic list [section]
topic sections
topic print <topic>...
topic search [--all] [expression]...
topic edit <topic>
topic version
```

Search is the default action. Command aliases are configurable, with `q` for
`search` and `p` for `print` enabled by default. Run `topic --help` for the
complete command and option reference.

Install the optimized release binary into `$BINDIR`, which defaults to
`$HOME/.local/bin`:

```sh
make -f .makefile install
```

`topic edit` launches `$VISUAL`, `$EDITOR`, or `vi`, refreshes the canonical
timestamp footer, and stages, commits, and pushes plaintext topics. Encrypted
topics are decrypted into a mode-0600 temporary file and re-encrypted for all
configured recipients; they are never staged or pushed.
