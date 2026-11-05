# ytd

A Rust wrapper around [yt-dlp](https://github.com/yt-dlp/yt-dlp), which must be
installed separately. It applies a saved download directory, file name template,
default options, and environment variables to each run. Configuration defaults
to `$XDG_CONFIG_HOME/etc/ytd.toml`, or `~/.config/etc/ytd.toml` when
`XDG_CONFIG_HOME` is unset, empty, or relative. `YTD_CONFIG` and `--config`
provide overrides.

The file is required. When it is missing, `ytd` stops with an error and shows
how to create it:

```sh
mkdir -p "$XDG_CONFIG_HOME/etc"
ytd init > "$XDG_CONFIG_HOME/etc/ytd.toml"
```

```text
ytd get [--dir DIR] [--audio] <url>... [-- <yt-dlp argument>...]
ytd list [--text] <url>... [-- <yt-dlp argument>...]
ytd init
ytd --version
```

Both commands accept `--config FILE`. `--version` prints the Git tag on the
commit that was built (or `dev`) and the 7-character commit SHA, such as
`ytd dev aa1af73`.

`get` downloads each URL. `--dir` replaces the configured download directory,
and `--audio` extracts the audio only.

`list` prints a playlist or channel as JSON, using yt-dlp's `--flat-playlist` and
`--dump-single-json`, without downloading anything. It prints one JSON document
per URL. `--text` prints a `title [id]` line per video instead.

Playlists can include videos that yt-dlp returns with only an `id` and
`url`. Those have no `title` in the JSON, and `--text` shows them as `NA`.

A URL can be a bare YouTube video ID. An 11-character ID that starts with `-` is
turned into a full URL, so that it isn't read as an option.

`init` prints a default configuration file to stdout, with every setting
commented out. It neither reads nor writes the configuration, so it works when
the file is missing or broken, as do `--help` and `--version`.

Anything after `--` goes to yt-dlp unchanged, after the configured `extra_args`:

```sh
ytd get https://example.com/watch?v=abc
ytd get --audio --dir ~/Music https://example.com/watch?v=abc
ytd get https://example.com/watch?v=abc -- --format 'bestvideo[height<=1080]+bestaudio'
ytd list https://example.com/playlist?list=abc
```

| Setting           | Default                      | Purpose                         |
| ----------------- | ---------------------------- | ------------------------------- |
| `yt_dlp_binary`   | `yt-dlp`                     | Program to run                  |
| `download_dir`    | `~/Downloads`                | Passed to yt-dlp as `--paths`   |
| `output_template` | `%(title)s [%(id)s].%(ext)s` | Passed to yt-dlp as `--output`  |
| `extra_args`      | none                         | Added to every yt-dlp command   |

`ytd` exits with status 1 if yt-dlp fails.

Settings come first in the file, ahead of the `[aliases]` and `[env]` tables:

```toml
extra_args = ["--embed-metadata"]

[aliases]
music = "get --audio --dir ~/Music"
"get hd" = "get -- --format bestvideo[height<=1080]+bestaudio"
ls = "list"

[env]
HTTPS_PROXY = "http://proxy.example.com:8080"
```

### Aliases

If the words after `ytd` start with a key in `[aliases]`, they are expanded as if
you had typed the right-hand side, so `ytd music <url>` runs
`ytd get --audio --dir ~/Music <url>`. The expansion is split on whitespace, with
no shell quoting.

- A key can be several words, and the longest matching key wins.
- A one-word key never hides `get`, `init`, `list`, or `help`.
- Only one level of expansion happens, so an alias can't resolve to another alias.
- If the expansion contains `--`, URLs from the command line go ahead of it.

### Environment variables

The `[env]` table sets default environment variables for yt-dlp.

A variable is used only when it is not already set in the environment, and a
variable set to an empty value counts as set.

Install the optimized release binary into `$BINDIR`, which defaults to
`$HOME/.local/bin`:

```sh
make -f .makefile install
```
