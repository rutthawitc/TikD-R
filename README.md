# TikD-R

> A fast, reliable Rust CLI for downloading TikTok videos without watermarks

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

TikD-R is a high-performance command-line tool written in Rust for downloading TikTok videos without watermarks. It handles the latest TikTok web flows, manages session cookies automatically, and organizes downloads by creator, making it perfect for archiving content without manual file management.

## Features

- **Watermark-Free Downloads** - Get clean videos without TikTok watermarks
- **Smart Organization** - Automatically sorts videos into folders by creator handle
- **Concurrent Downloads** - Download multiple videos simultaneously with configurable concurrency
- **Resilient Retries** - Automatic retry logic with exponential backoff for transient failures
- **HLS Support** - Falls back to HLS streaming when direct downloads aren't available
- **Batch Processing** - Process multiple URLs from a file with a single command
- **Cookie Management** - Maintains session cookies automatically for seamless downloads
- **Cross-Platform** - Works on Linux, macOS, and Windows

## Installation

### Option 1: Install from Source (Recommended)

Requires [Rust toolchain](https://rustup.rs/) 1.70 or later:

```bash
git clone https://github.com/rutthawitc/TikD-R.git
cd TikD-R
cargo install --path .
```

This builds and installs the `tikd-r` binary to `~/.cargo/bin/`. Make sure this directory is in your `PATH`.

### Option 2: Build Release Binary

```bash
cargo build --release
```

The optimized binary will be available at `target/release/tikd-r`. You can move it to a location in your `PATH`:

```bash
# Linux/macOS
sudo mv target/release/tikd-r /usr/local/bin/

# Or to your local bin
mv target/release/tikd-r ~/.local/bin/
```

### Option 3: Run Directly from Repository

For development or testing:

```bash
cargo run --release -- <ARGS>
```

### Option 4: Download Pre-built Binaries

Check the [GitHub Releases](https://github.com/rutthawitc/TikD-R/releases) page for pre-built binaries:
- macOS (Apple Silicon & Intel)
- Linux (x64)
- Windows (x64)

Download the appropriate archive, extract it, and add the executable to your `PATH`.

## Usage

### Quick Start

Download a single video:

```bash
tikd-r https://vt.tiktok.com/ZSyB3RCuJ/
```

This follows TikTok's shortlink, extracts the video ID and creator handle, and saves it as:
```
frictionlesson/7551290370794016007.mp4
```

Videos are automatically organized into folders named after the creator's handle.

### Single Video Download

```bash
# Using short URL
tikd-r https://vt.tiktok.com/ZSyB3RCuJ/

# Using full URL
tikd-r https://www.tiktok.com/@username/video/1234567890123456789
```

### Batch Downloads

Create a text file with one URL per line:

```txt
# urls.txt
https://vt.tiktok.com/ZSyB3RCuJ/
https://www.tiktok.com/@another_creator/video/1234567890123456789
https://www.tiktok.com/@user/video/9876543210987654321

# This is a comment - lines starting with # are ignored
```

Then run:

```bash
tikd-r --file urls.txt
```

**Batch Mode Features:**
- Comments: Lines starting with `#` are ignored
- Blank lines are skipped
- Duplicate URLs are automatically removed
- Failed downloads don't stop the batch - the summary shows successes and failures
- Resume interrupted batches by running the same command again

### Advanced Configuration

#### Tuning Concurrency

Control how many downloads run simultaneously (default: 4):

```bash
tikd-r --file urls.txt --max-concurrent 8
```

**Recommendations:**
- Use lower values (2-4) if you experience rate limiting
- Increase (6-10) if you have high bandwidth and stable connection
- TikTok may throttle aggressive download rates

#### Retry Configuration

Customize retry behavior for transient failures (default: 3 retries):

```bash
tikd-r --file urls.txt --max-retries 5 --backoff-ms 750
```

**Parameters:**
- `--max-retries NUM`: Maximum retry attempts for failed downloads
- `--backoff-ms MS`: Initial backoff delay in milliseconds (doubles with each retry)

**Example:** With `--max-retries 3 --backoff-ms 500`, failures are retried after 500ms, 1000ms, and 2000ms.

#### Complete Example

```bash
tikd-r --file urls.txt \
  --max-concurrent 6 \
  --max-retries 5 \
  --backoff-ms 750
```

### Output Format

Successful downloads display:
```
Downloaded https://vt.tiktok.com/ZSyB3RCuJ/ -> frictionlesson/7551290370794016007.mp4
```

Failed downloads show errors:
```
Failed https://vt.tiktok.com/invalid/: Video not found
```

Final summary:
```
Summary: 5 succeeded, 1 failed.
```

### Debug Logging

Enable detailed logging for troubleshooting:

```bash
RUST_LOG=tikd_r=debug tikd-r https://vt.tiktok.com/ZSyB3RCuJ/
```

This shows:
- URL resolution and redirects
- Video metadata extraction
- Download method (direct binary vs HLS streaming)
- HLS playlist parsing and segment downloads
- Retry attempts and backoff timing
- Error context and stack traces

## How It Works

1. **URL Resolution**: Follows TikTok short URLs to canonical video pages
2. **Metadata Extraction**: Scrapes video metadata including creator handle and video ID
3. **Download Strategy**:
   - Attempts direct binary download first (fastest)
   - Falls back to HLS streaming if needed (assembles video from segments)
4. **Organization**: Creates folders by creator handle and names files by video ID
5. **Error Handling**: Retries transient failures (403, 429, 5xx) with exponential backoff

## Project Structure

```
TikD-R/
├── src/
│   ├── main.rs         # CLI entry point
│   ├── cli.rs          # Command-line argument parsing
│   ├── downloader.rs   # Download orchestration and HTTP client
│   ├── scraper.rs      # TikTok HTML parsing and video extraction
│   ├── error.rs        # Error types and handling
│   └── lib.rs          # Library exports
├── tests/              # Integration tests
├── Cargo.toml          # Rust dependencies and metadata
└── README.md           # This file
```

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
```

### Testing

```bash
# Run all tests
cargo test

# Run with live integration tests (requires TIKD_TEST_URL environment variable)
cargo test --features live-tests
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features

# Check for issues without building
cargo check
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:
- Development guidelines and workflow
- Code style conventions
- Testing requirements
- Pull request process

## Notes and Limitations

### Folder Organization
- Videos are saved to folders named after the creator's `@handle`
- If the handle can't be determined, videos go to an `unknown/` folder
- This makes it easy to organize large archives by creator

### Batch Processing
- Empty lines and lines starting with `#` are ignored in batch files
- Duplicate URLs are automatically removed before downloading
- Partial failures don't stop the batch - check the summary at the end

### Rate Limiting
- TikTok may throttle or block aggressive download patterns
- Use `--max-concurrent` to reduce load if you encounter issues
- Consider adding delays between large batch jobs

### Video Format
- Downloads MP4 format when available
- HLS streaming support for videos that require it
- Encrypted videos (AES-128, SAMPLE-AES) are not currently supported

### Privacy and Authentication
- Some videos may require authentication or have geographic restrictions
- The tool maintains session cookies but doesn't handle login flows
- Private accounts and age-restricted content may not be accessible

### Live Testing
- Integration tests are gated behind the `live-tests` feature flag
- Set `TIKD_TEST_URL` environment variable to test with a real TikTok URL
- Avoids publishing test URLs in the public repository

## Troubleshooting

### "Failed to download: Video not found"
- The video may have been deleted or made private
- Check if the URL is accessible in a browser
- Some videos may be geo-restricted

### "Rate limited" or 429 errors
- Reduce `--max-concurrent` to 2-3
- Increase `--backoff-ms` to 1000 or higher
- Wait a few minutes before retrying

### HLS download failures
- Enable debug logging: `RUST_LOG=tikd_r=debug`
- Check if segments are accessible
- Some videos may use encryption (not yet supported)

### Binary contains garbage in URL
- This was a bug in earlier versions (now fixed)
- Update to the latest version
- The tool now properly detects binary vs HLS content

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This tool is for educational and personal use only. Users are responsible for complying with TikTok's Terms of Service and applicable copyright laws. The authors are not responsible for any misuse of this software.

## Acknowledgments

Built with:
- [Tokio](https://tokio.rs/) - Async runtime
- [Reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [Scraper](https://github.com/causal-agent/scraper) - HTML parsing
- [Clap](https://github.com/clap-rs/clap) - CLI argument parsing

## Support

- Report issues: [GitHub Issues](https://github.com/rutthawitc/TikD-R/issues)
- Submit pull requests: [GitHub Pull Requests](https://github.com/rutthawitc/TikD-R/pulls)
- Documentation: [Project Wiki](https://github.com/rutthawitc/TikD-R/wiki)

---

Made with ❤️ and Rust
