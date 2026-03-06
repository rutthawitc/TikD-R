# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
cargo build                              # Debug build
cargo build --release                    # Release build
cargo run -- <VIDEO_URL>                 # Run with a single URL
cargo run -- --file urls.txt             # Run with batch file
cargo test                               # Run all unit + integration tests
cargo test --features live-tests         # Include live integration tests (requires TIKD_R_LIVE_URL env var)
cargo test <test_name>                   # Run a single test by name
cargo fmt                                # Format code
cargo clippy --all-targets --all-features  # Lint
cargo check                              # Type-check without building
```

Debug logging: `RUST_LOG=tikd_r=debug cargo run -- <URL>`

## Architecture

TikD-R is a Rust CLI tool for downloading TikTok videos without watermarks. It's built as both a binary and a library (`lib.rs` re-exports all public modules).

### Data Flow

`main.rs` (CLI parsing + URL gathering) -> `Downloader::download_all` (concurrent orchestration with retry) -> `Scraper::extract_video_descriptor` (HTML scraping) -> binary download or HLS fallback -> file written to `{author_handle}/{video_id}.mp4`

### Module Responsibilities

- **`cli.rs`** - Clap derive-based argument parsing. URL and `--file` are mutually exclusive inputs.
- **`scraper.rs`** - Extracts `VideoDescriptor` (video_id, download_url, play_url, author) from TikTok HTML. Tries three JSON extraction strategies in order: `__UNIVERSAL_DATA_FOR_REHYDRATION__` -> `SIGI_STATE` -> `__NEXT_DATA__`. Falls back to URL path parsing for video ID and author.
- **`downloader.rs`** - Core download logic. `Downloader` wraps a shared `reqwest::Client` with cookie store. Download strategy: try direct binary download first, fall back to HLS streaming (master playlist -> variant selection -> segment assembly). Includes retry with exponential backoff and configurable concurrency via `futures::stream::buffer_unordered`.
- **`error.rs`** - Single `Error` enum using `thiserror`. `Result<T>` type alias used throughout.

### Key Design Decisions

- Cookie persistence via `reqwest_cookie_store` for session management across requests
- `rustls-tls` (not native TLS) for cross-platform compatibility
- Output path sanitization handles Windows-reserved filenames and characters
- Retry logic distinguishes transient errors (network, 403, 429, 5xx) from permanent ones (invalid URL, missing input)
- HLS fallback detects binary video content via Content-Type to avoid misinterpreting MP4 data as playlist text

## Testing

- Unit tests are co-located in each source file under `#[cfg(test)]`
- Integration tests in `tests/cli.rs` (CLI validation) and `tests/live.rs` (real TikTok fetch, gated by `live-tests` feature flag)
- Test fixtures in `tests/fixtures/` contain sample HTML for scraper tests
- Live tests require `TIKD_R_LIVE_URL` env var; optionally `TIKD_R_EXPECT_VIDEO_ID` for assertion

## Conventions

- Rust 2021 edition, MSRV 1.70
- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`
- Imports ordered: std -> external crates -> internal modules
- No `unwrap()` in production code; use `Result` and `?` operator
- Async runtime: Tokio (full features)
