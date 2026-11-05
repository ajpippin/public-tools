# ytd

A Rust wrapper around [yt-dlp](https://github.com/yt-dlp/yt-dlp), which must be
installed separately. It applies a saved download directory, file name template,
and default options to each download. Configuration defaults to
`~/.config/tools/ytd.yaml`; `YTD_CONFIG` and `--config` provide overrides.

When this file is missing, `ytd` creates it automatically with every setting
shown as a commented-out default. Existing files are never rewritten during
startup.

```text
ytd get [--dir DIR] [--audio] <url>... [-- <yt-dlp argument>...]
ytd list <url>... [-- <yt-dlp argument>...]
ytd --version
```

Both commands accept `--config FILE`. `--version` prints the Git tag on the
commit that was built (or `dev`) and the 7-character commit SHA, such as
`ytd dev aa1af73`.

`get` downloads each URL. `--dir` replaces the configured download directory,
and `--audio` extracts the audio only.

`list` prints the title and ID of each video in a playlist or channel, without
downloading anything. A URL for a single video prints one line.

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

Install the optimized release binary into `$BINDIR`, which defaults to
`$HOME/.local/bin`:

```sh
make -f .makefile install
```
