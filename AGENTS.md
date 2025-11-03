# Repository Guidelines

## Project Structure & Module Organization
`TikD-R PRD.md` and `TikD-R System Design.md` document the current scope; keep them updated when requirements shift. When the Rust crate is scaffolded, follow the layout outlined in the system design (`Cargo.toml`, `src/`, `tests/`, optional `assets/`). Group async networking in `src/downloader.rs`, scraping logic in `src/scraper.rs`, CLI setup in `src/main.rs`, and shared error types in `src/error.rs` to keep failure modes isolated.

## Build, Test, and Development Commands
- `cargo build --release` produces an optimized `tikd-r` binary for distribution.
- `cargo run -- <VIDEO_URL>` exercises the CLI end-to-end; use `--file urls.txt` for batch runs.
- `cargo fmt && cargo clippy --all-targets --all-features` enforces formatting and lints before opening a pull request.
- `cargo test --all` runs unit tests and any async integration suites under `tests/`.

## Coding Style & Naming Conventions
Adopt Rust 2021 defaults: four-space indentation, `rustfmt` formatting, and imports grouped by standard/library/crate. Prefer `snake_case` for functions and variables, `CamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Keep modules small; if a file exceeds ~300 lines, split it under `src/` with a `mod.rs` to re-export the public API.

## Testing Guidelines
Add focused unit tests alongside the module under test and async integration tests under `tests/`. Cover both success paths (video download, file naming) and failures (invalid URL, network timeout). Use `#[tokio::test]` for async cases and include fixture URLs in `tests/fixtures/` once assets are needed.

## Commit & Pull Request Guidelines
Write imperative, 72-character subject lines (for example, `Add scraper module for video URL extraction`) with concise bodies explaining rationale and risk. Reference related issues with `Closes #ID` when applicable. Pull requests should summarize behavior changes, list manual test results (`cargo test`, sample download command), and include screenshots or logs if they clarify error handling. Tag reviewers responsible for the affected modules and confirm lint/test checks pass before requesting review.

## Security & Configuration Notes
Never commit API keys or TikTok cookies; rely on environment variables or `~/.config/tikd-r/config.toml` (ignored by git) if persistent configuration becomes necessary. Validate all user-provided URLs before issuing network calls, and avoid writing files outside the working directory to minimize accidental overwrites.
