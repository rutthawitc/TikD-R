# TikD-R

TikD-R is a Rust CLI for downloading TikTok videos without watermarks. It speaks the latest web flows, maintains session cookies automatically, and writes each download into a per-creator directory so you can archive feeds without manual sorting.

## Installation

| Option | Command | Notes |
| --- | --- | --- |
| **Install from source (Rust toolchain required)** | `cargo install --path .` | Builds a `tikd-r` binary in `~/.cargo/bin`. |
| **Run directly from the repo** | `cargo run --release -- <ARGS>` | Good while iterating on the codebase. |
| **Download a release binary** | See the [GitHub Releases](https://github.com/your-org/tikd-r/releases) page | macOS (Apple/Intel), Linux x64, and Windows x64 archives include the `tikd-r` executable. Add it to your `PATH` manually. |

To create release-ready binaries yourself:

```bash
cargo build --release       # builds target/release/tikd-r
```

For Windows cross-builds on macOS/Linux you can use `cross` or GitHub Actions. The existing CI workflow already compiles and tests on Ubuntu and can be extended to upload artifacts.

## Usage

### Single URL

```bash
cargo run -- https://vt.tiktok.com/ZSyB3RCuJ/
```

The command above follows TikTok’s mobile shortlink and writes `frictionlesson/7551290370794016007.mp4` (or the resolved creator/video id pair) to the current working directory.

### Batch Mode

Place one URL per line in a text file and pass it with `--file`:

```
# urls.txt
https://vt.tiktok.com/ZSyB3RCuJ/
https://www.tiktok.com/@another_creator/video/1234567890123456789
```

```bash
cargo run -- --file urls.txt
```

Each downloaded video is streamed directly to disk. Even if a URL fails, the CLI keeps going and finishes with a summary such as `Summary: 5 succeeded, 1 failed.` The failing URLs stay in the output so you can retry them.

### Tuning Throughput

The downloader runs four requests at a time and retries transient 403/429/5xx responses up to three times with exponential backoff by default. Adjust those limits on the command line:

```bash
cargo run -- --file urls.txt \
  --max-concurrent 6 \
  --max-retries 5 \
  --backoff-ms 750
```

Use smaller concurrency or longer backoff if TikTok throttles your IP; increase them if you have plenty of bandwidth.

## Notes

- Output folders are derived from the creator `@handle`. Unknown handles fall back to `unknown/`.
- Input files ignore blank lines and lines starting with `#`.
- Live integration tests are gated behind the `live-tests` Cargo feature—you can supply a private URL in CI to monitor TikTok layout changes without publishing it publicly.
