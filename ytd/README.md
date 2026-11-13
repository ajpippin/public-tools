# ytd

A Rust wrapper around [yt-dlp](https://github.com/yt-dlp/yt-dlp), which must be
installed separately. It applies a saved download directory, file name template,
default options, and environment variables to each run. `YTD_CONFIG` and
`--config` choose the configuration file. Without an override, the configuration is the first of these that exists:

1. `$XDG_CONFIG_HOME/tools/ytd.toml`
2. `~/.config/tools/ytd.toml`
3. `$XDG_CONFIG_HOME/ytd.toml`
4. `~/.config/ytd.toml`

`XDG_CONFIG_HOME` is skipped when it is unset, empty, or relative.

The file is required. When none of them exists, `ytd` stops with an error and
shows how to create the first one, such as:

```sh
mkdir -p ~/.config/tools
ytd init > ~/.config/tools/ytd.toml
```

```text
ytd get [--dir DIR] [--type TYPES] [--video] [--audio] [--transcript] [--lang LANGS] <url>... [-- <yt-dlp argument>...]
ytd list [--text] <url>... [-- <yt-dlp argument>...]
ytd init
ytd --version
```

Every command accepts `--config FILE`. `--version` prints the Git tag on the
commit that was built (or `dev`) and the 7-character commit SHA, such as
`ytd dev aa1af73`.

`get` downloads each URL. `--dir` replaces the configured download directory.
`--type` chooses what to save, and takes one or more of these, separated by
commas or given as repeated `--type` options:

| Type  | Saves                                                         |
| ----- | ------------------------------------------------------------- |
| `mp4` | The video, as MP4 (the default)                               |
| `mp3` | The audio, as MP3                                             |
| `txt` | The captions, as plain text                                   |
| `all` | All three                                                     |

`--video` is the same as `--type mp4`, `--audio` is the same as `--type mp3`, and
`--transcript` is the same as `--type txt`. They add to `--type`, so
`--audio --transcript` saves the audio and the captions. Each type is a separate yt-dlp run, in the order `mp4`, `mp3`,
`txt`, and `ytd` stops at the first run that fails. The arguments after `--` go
to every run.

With both `mp4` and `mp3`, the video is downloaded once and ffmpeg makes the MP3
from it, beside the video, and `ytd` prints the MP3's path. This also works when
the video was downloaded before.

A video URL within a playlist downloads only that video, and a playlist URL
still downloads the playlist. Add `-- --yes-playlist` to download the whole
playlist from a video URL.

`list` prints a playlist or channel as JSON, using yt-dlp's `--flat-playlist` and
`--dump-single-json`, without downloading anything. It prints one JSON document
per URL. `--text` prints a `title [id]` line per video instead.

`list` also takes the URL of a video within a playlist, such as one with
`watch?v=...&list=...`, and lists the playlist from its `list=` value. Quote a
URL like that, because the shell treats `?` and `&` as special characters.

Playlists can include videos that yt-dlp returns with only an `id` and
`url`. Those have no `title` in the JSON, and `--text` shows them as `NA`.

`get --transcript` (or `--type txt`) saves a video's captions as plain text, without the timings or the repeated lines of auto-generated captions,
and prints each file's path. The file goes in the download directory, as
`<name>.<language>.txt`. It uses uploaded captions when a video has them, and
auto-generated ones otherwise. `--lang` defaults to `en` and takes what yt-dlp's
`--sub-langs` takes, such as `en.*` or `en,fr`, and is an error without `txt`. It stops with an error if nothing
is available.

A URL can be a bare YouTube video ID. An 11-character ID that starts with `-` is
turned into a full URL, so that it isn't read as an option.

`init` prints a default configuration file to stdout, with every setting
commented out. It neither reads nor writes the configuration, so it works when
the file is missing or broken, as do `--help` and `--version`.

Anything after `--` goes to yt-dlp unchanged, after the configured `extra_args`:

```sh
ytd get https://example.com/watch?v=abc
ytd get --audio --dir ~/Music https://example.com/watch?v=abc
ytd get --type mp4,txt https://example.com/watch?v=abc
ytd get --type=all https://example.com/watch?v=abc
ytd get https://example.com/watch?v=abc -- --format 'bestvideo[height<=1080]+bestaudio'
ytd list https://example.com/playlist?list=abc
```

| Setting           | Default                      | Purpose                         |
| ----------------- | ---------------------------- | ------------------------------- |
| `yt_dlp_binary`   | `yt-dlp`                     | Program to run                  |
| `ffmpeg_binary`   | `ffmpeg`                     | Makes MP3s from downloaded MP4s |
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
transcript = "get --transcript"

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
