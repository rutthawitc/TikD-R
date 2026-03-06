# TikD-R

> A fast, reliable Rust CLI for downloading TikTok videos without watermarks

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

TikD-R is a high-performance command-line tool written in Rust for downloading TikTok videos without watermarks. It handles the latest TikTok web flows, manages session cookies automatically, and organizes downloads by creator, making it perfect for archiving content without manual file management.

## Features

- **Watermark-Free Downloads** — Get clean videos without TikTok watermarks
- **Smart Organization** — Automatically sorts videos into folders by creator handle
- **Concurrent Downloads** — Download multiple videos simultaneously with configurable concurrency
- **Resilient Retries** — Automatic retry logic with exponential backoff for transient failures
- **HLS Support** — Falls back to HLS streaming when direct downloads aren't available
- **Batch Processing** — Process multiple URLs from a file with real-time progress
- **Resume Support** — Already-downloaded files are skipped automatically
- **Custom Output Directory** — Save videos to any directory with `-o`/`--output-dir`
- **Cookie Management** — Maintains session cookies automatically for seamless downloads
- **Cross-Platform** — Works on Linux, macOS, and Windows (with proper filename sanitization)

## Installation

### From Source (Recommended)

Requires [Rust toolchain](https://rustup.rs/) 1.70 or later:

```bash
git clone https://github.com/rutthawitc/TikD-R.git
cd TikD-R
cargo install --path .
```

This builds and installs the `tikd-r` binary to `~/.cargo/bin/`. Make sure this directory is in your `PATH`.

### Build Release Binary

```bash
cargo build --release
```

The optimized binary will be at `target/release/tikd-r`:

```bash
# Linux/macOS
sudo mv target/release/tikd-r /usr/local/bin/

# Or to your local bin
mv target/release/tikd-r ~/.local/bin/
```

### Download Pre-built Binaries

Check the [GitHub Releases](https://github.com/rutthawitc/TikD-R/releases) page for pre-built binaries:
- macOS (Apple Silicon & Intel)
- Linux (x64)
- Windows (x64)

## Usage

### Quick Start

```bash
# Download a single video
tikd-r https://vt.tiktok.com/ZSyB3RCuJ/
```

Output:
```
Downloaded https://vt.tiktok.com/ZSyB3RCuJ/ -> frictionlesson/7551290370794016007.mp4
Summary: 1 succeeded, 0 failed.
```

Videos are automatically organized into folders named after the creator's handle.

### Command Reference

```
tikd-r [OPTIONS] [VIDEO_URL]
```

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `VIDEO_URL` | | Single TikTok video URL to download | — |
| `--file <PATH>` | | File with line-delimited URLs for batch downloads | — |
| `--output-dir <DIR>` | `-o` | Output directory for downloaded videos | Current directory |
| `--max-concurrent <NUM>` | | Maximum number of concurrent downloads | `4` |
| `--max-retries <NUM>` | | Maximum retry attempts per URL on transient failures | `3` |
| `--backoff-ms <MS>` | | Initial backoff delay in milliseconds (doubles each retry) | `500` |

> **Note:** `VIDEO_URL` and `--file` are mutually exclusive — use one or the other.

### Single Video Download

```bash
# Using short URL
tikd-r https://vt.tiktok.com/ZSyB3RCuJ/

# Using full URL
tikd-r https://www.tiktok.com/@username/video/1234567890123456789

# Save to a specific directory
tikd-r -o ~/Videos/TikTok https://vt.tiktok.com/ZSyB3RCuJ/
```

### Batch Downloads

Create a text file with one URL per line:

```txt
# urls.txt
https://vt.tiktok.com/ZSyB3RCuJ/
https://www.tiktok.com/@another_creator/video/1234567890123456789
https://www.tiktok.com/@user/video/9876543210987654321

# Lines starting with # are comments and will be ignored
```

Then run:

```bash
tikd-r --file urls.txt
```

**Batch mode features:**
- Lines starting with `#` are comments (ignored)
- Blank lines are skipped
- Duplicate URLs are automatically removed
- Real-time progress: `[1/5] url ... ok` as each download completes
- Failed downloads don't stop the batch — the summary shows results
- Already-downloaded files are skipped (resume interrupted batches)

**Example batch output:**
```
[1/3] https://vt.tiktok.com/ZSyB3RCuJ/ ... ok
[2/3] https://www.tiktok.com/@user/video/123 ... ok
[3/3] https://www.tiktok.com/@user/video/456 ... FAILED
Downloaded https://vt.tiktok.com/ZSyB3RCuJ/ -> frictionlesson/7551290370794016007.mp4
Downloaded https://www.tiktok.com/@user/video/123 -> user/123.mp4
Failed https://www.tiktok.com/@user/video/456: Video not found
Summary: 2 succeeded, 1 failed.
```

### Output Directory

By default, videos are saved in the current working directory. Use `-o` / `--output-dir` to specify a different location:

```bash
# Save to ~/Videos/TikTok
tikd-r -o ~/Videos/TikTok https://vt.tiktok.com/ZSyB3RCuJ/
# Result: ~/Videos/TikTok/frictionlesson/7551290370794016007.mp4

# Batch download to a specific directory
tikd-r --file urls.txt -o ~/Videos/TikTok
```

The directory (and any necessary subdirectories) will be created automatically if it doesn't exist.

### Tuning Concurrency

Control how many downloads run simultaneously (default: 4):

```bash
tikd-r --file urls.txt --max-concurrent 8
```

**Recommendations:**
- Use lower values (2–4) if you experience rate limiting
- Increase (6–10) if you have high bandwidth and a stable connection
- TikTok may throttle aggressive download rates

### Retry Configuration

Customize retry behavior for transient failures (default: 3 retries, 500ms initial backoff):

```bash
tikd-r --file urls.txt --max-retries 5 --backoff-ms 750
```

The backoff doubles with each retry. For example, `--max-retries 3 --backoff-ms 500` retries after 500ms, 1000ms, and 2000ms.

Retried errors include: network timeouts, connection failures, HTTP 403/429, and server errors (5xx). Permanent errors (invalid URL, video not found) are not retried.

### Complete Example

```bash
tikd-r --file urls.txt \
  -o ~/Videos/TikTok \
  --max-concurrent 6 \
  --max-retries 5 \
  --backoff-ms 750
```

### Debug Logging

Enable detailed logging for troubleshooting:

```bash
RUST_LOG=tikd_r=debug tikd-r https://vt.tiktok.com/ZSyB3RCuJ/
```

This shows:
- URL resolution and redirects
- Video metadata extraction
- Download method selection (direct binary vs HLS streaming)
- HLS playlist parsing and segment downloads
- Retry attempts and backoff timing
- File skip decisions (already downloaded)

## How It Works

1. **URL Resolution** — Follows TikTok short URLs (e.g., `vt.tiktok.com/...`) through redirects to the canonical video page
2. **Metadata Extraction** — Parses the page HTML looking for embedded JSON data in three formats:
   - `__UNIVERSAL_DATA_FOR_REHYDRATION__` (current TikTok format)
   - `SIGI_STATE` (older format)
   - `__NEXT_DATA__` (legacy format)
3. **Skip Check** — If the output file already exists and is non-empty, the download is skipped
4. **Download Strategy**:
   - Attempts direct binary download first (fastest, single HTTP request)
   - Validates response Content-Type to detect error pages served as HTML
   - Falls back to HLS streaming if direct download fails (fetches master playlist, selects highest bandwidth variant, downloads and assembles segments)
   - HLS segment downloads include their own retry logic
5. **File Organization** — Creates folders by creator handle (`@username` → `username/`) and names files by video ID (`username/7551290370794016007.mp4`). If the handle can't be determined, videos go to `unknown/`
6. **Error Handling** — Retries transient failures (403, 429, 5xx, timeouts) with exponential backoff. Permanent errors fail immediately
7. **Batch Orchestration** — Downloads run concurrently using async streams with configurable parallelism. Progress is reported in real-time as each download completes

## Development

### Prerequisites

- Rust 1.70 or later
- `cargo` build tool

### Building

```bash
# Debug build (faster compilation, slower execution)
cargo build

# Release build (optimized)
cargo build --release

# Run directly from source
cargo run --release -- <ARGS>
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test sanitize_strips

# Run with live integration tests (requires real TikTok URL)
TIKD_R_LIVE_URL="https://vt.tiktok.com/..." cargo test --features live-tests

# Optionally assert a specific video ID
TIKD_R_LIVE_URL="https://vt.tiktok.com/..." TIKD_R_EXPECT_VIDEO_ID="123456" cargo test --features live-tests
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features

# Type-check without building
cargo check
```

## Troubleshooting

### "Video not found" errors
- The video may have been deleted or made private
- Check if the URL is accessible in a browser
- Some videos may be geo-restricted

### Rate limiting (429 errors)
- Reduce `--max-concurrent` to 2–3
- Increase `--backoff-ms` to 1000 or higher
- Wait a few minutes before retrying

### HLS download failures
- Enable debug logging: `RUST_LOG=tikd_r=debug`
- Some videos may use encryption (AES-128, SAMPLE-AES) which is not yet supported
- HLS segments are retried individually on transient failures

### "Server returned HTML instead of video content"
- TikTok returned an error page instead of the video
- The video may require authentication or be region-locked
- Try again later — this is sometimes a transient issue

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines, code style conventions, testing requirements, and the pull request process.

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Disclaimer

This tool is for educational and personal use only. Users are responsible for complying with TikTok's Terms of Service and applicable copyright laws. The authors are not responsible for any misuse of this software.

## Acknowledgments

Built with:
- [Tokio](https://tokio.rs/) — Async runtime
- [Reqwest](https://github.com/seanmonstar/reqwest) — HTTP client
- [Scraper](https://github.com/causal-agent/scraper) — HTML parsing
- [Clap](https://github.com/clap-rs/clap) — CLI argument parsing

---

Made with Rust
