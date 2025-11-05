# Contributing to TikD-R

Thank you for considering contributing to TikD-R! This document provides guidelines and best practices for contributing to the project.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Security Considerations](#security-considerations)

## Getting Started

### Prerequisites

- Rust 1.70 or later ([install via rustup](https://rustup.rs/))
- Git for version control
- Familiarity with async Rust (Tokio)

### Setting Up Your Development Environment

1. Fork the repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/TikD-R.git
   cd TikD-R
   ```
3. Add the upstream repository:
   ```bash
   git remote add upstream https://github.com/rutthawitc/TikD-R.git
   ```
4. Build the project:
   ```bash
   cargo build
   ```
5. Run tests to ensure everything works:
   ```bash
   cargo test
   ```

## Development Workflow

### Before Starting Work

1. Sync with upstream:
   ```bash
   git fetch upstream
   git checkout main
   git merge upstream/main
   ```

2. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature-name
   ```

### During Development

1. Make your changes following the coding standards below
2. Add or update tests as needed
3. Run the test suite frequently:
   ```bash
   cargo test
   ```
4. Format and lint your code:
   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features
   ```

### Quick Commands Reference

```bash
# Build (debug mode - faster compilation)
cargo build

# Build (release mode - optimized)
cargo build --release

# Run the CLI
cargo run -- <VIDEO_URL>
cargo run -- --file urls.txt

# Run tests
cargo test --all

# Run tests with live integration tests
cargo test --features live-tests

# Format code
cargo fmt

# Run linter
cargo clippy --all-targets --all-features

# Check for issues without building
cargo check
```

## Project Structure

The project follows a modular architecture with clear separation of concerns:

```
TikD-R/
├── src/
│   ├── main.rs         # CLI entry point, argument parsing, orchestration
│   ├── cli.rs          # Command-line argument definitions (clap)
│   ├── lib.rs          # Library exports for public API
│   ├── downloader.rs   # HTTP client, concurrent downloads, retry logic
│   ├── scraper.rs      # TikTok HTML parsing, video URL extraction
│   └── error.rs        # Error types and result definitions
├── tests/              # Integration tests
│   └── integration_test.rs
├── Cargo.toml          # Dependencies and project metadata
└── Cargo.lock          # Locked dependency versions
```

### Module Responsibilities

- **`main.rs`**: Orchestrates the CLI flow, gathers URLs, calls the downloader
- **`cli.rs`**: Defines command-line arguments using clap's derive API
- **`downloader.rs`**: Manages async HTTP requests, concurrency, retries, HLS fallback
- **`scraper.rs`**: Parses TikTok HTML to extract video metadata and download URLs
- **`error.rs`**: Centralized error handling with `thiserror` for better error messages

### When to Split Modules

If a module file exceeds ~300-400 lines, consider splitting it:
- Create a subdirectory (e.g., `src/scraper/`)
- Use `mod.rs` to re-export the public API
- Keep related functionality grouped together

## Coding Standards

### Rust Edition and Formatting

- Use **Rust 2021 edition**
- **4-space indentation** (enforced by `rustfmt`)
- Maximum line length: **100 characters** (soft limit)
- Use `cargo fmt` before committing

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Functions/Variables | `snake_case` | `download_video()`, `max_retries` |
| Types/Structs/Enums | `PascalCase` | `DownloadConfig`, `Error` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_TIMEOUT` |
| Modules | `snake_case` | `downloader`, `scraper` |
| Lifetimes | single letter or short | `'a`, `'src` |

### Import Organization

Group imports in this order:
1. Standard library (`std::`, `core::`)
2. External crates (alphabetical)
3. Internal crates/modules (alphabetical)

Example:
```rust
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use reqwest::Client;
use scraper::Html;

use crate::error::{Error, Result};
use crate::scraper::extract_video;
```

### Code Style Best Practices

- **Prefer explicit over implicit**: Use descriptive variable names
- **Keep functions focused**: Single responsibility principle
- **Document public APIs**: Use `///` doc comments for public items
- **Handle errors explicitly**: Don't use `unwrap()` in production code
- **Use `Result` and `?` operator**: For error propagation
- **Async functions**: Prefix with clear async semantics

Example:
```rust
/// Downloads a TikTok video from the given URL.
///
/// # Arguments
/// * `url` - The TikTok video URL to download
///
/// # Returns
/// * `Ok(PathBuf)` - Path to the downloaded video file
/// * `Err(Error)` - Download error with context
pub async fn download_video(url: &str) -> Result<PathBuf> {
    // Implementation
}
```

## Testing Guidelines

### Test Organization

- **Unit tests**: Place in the same file as the code under test
- **Integration tests**: Place in `tests/` directory
- **Test fixtures**: Store sample data in `tests/fixtures/` if needed

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_parsing() {
        let url = "https://www.tiktok.com/@user/video/123";
        let result = parse_url(url);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_async_download() {
        let result = download_video("https://...").await;
        assert!(result.is_ok());
    }
}
```

#### Integration Tests

- Use `#[tokio::test]` for async tests
- Test end-to-end scenarios
- Include both success and failure cases
- Use `tempfile` for file system tests

### Test Coverage Goals

- **Unit tests**: Cover all public functions
- **Integration tests**: Cover main user workflows
- **Error paths**: Test failure modes and error messages
- **Edge cases**: Empty inputs, invalid URLs, network failures

### Live Integration Tests

For testing with real TikTok URLs (optional):
```bash
# Set test URL in environment
export TIKD_TEST_URL="https://vt.tiktok.com/..."

# Run with live-tests feature
cargo test --features live-tests
```

**Note**: Never commit real TikTok URLs to the repository. Use environment variables.

## Commit Guidelines

### Commit Message Format

Use conventional commit format:

```
<type>: <subject>

<body>

<footer>
```

#### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style/formatting (no logic change)
- `refactor`: Code restructuring (no behavior change)
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks, dependencies

#### Subject Line

- Use imperative mood: "Add feature" not "Added feature"
- Maximum 72 characters
- No period at the end
- Capitalize first letter

#### Body (optional)

- Explain what and why, not how
- Wrap at 72 characters
- Separate from subject with blank line

#### Footer (optional)

- Reference issues: `Closes #123`
- Breaking changes: `BREAKING CHANGE: description`

### Examples

```
feat: Add HLS streaming fallback for video downloads

Implements automatic fallback to HLS streaming when direct
binary download fails. This handles cases where TikTok serves
videos as segmented streams instead of single MP4 files.

Closes #42
```

```
fix: Handle binary video content misinterpretation

Previously, the downloader attempted to parse binary MP4 data
as HLS playlists, resulting in malformed URLs. Now detects
Content-Type and MP4 file signatures to choose the correct
download strategy.

Fixes #56
```

## Pull Request Process

### Before Submitting

1. **Update from main**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run full test suite**:
   ```bash
   cargo test --all
   cargo clippy --all-targets --all-features
   cargo fmt -- --check
   ```

3. **Test the CLI manually**:
   ```bash
   cargo run -- https://vt.tiktok.com/... # test URL
   cargo run -- --file test_urls.txt
   ```

### Pull Request Template

**Title**: Clear, descriptive summary (50 chars or less)

**Description**:
```markdown
## Summary
Brief description of changes

## Changes Made
- Added feature X
- Fixed bug Y
- Updated documentation Z

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed
- [ ] Clippy warnings resolved
- [ ] Code formatted with rustfmt

## Test Results
```bash
$ cargo test
test result: ok. 15 passed; 0 failed; 0 ignored
```

## Related Issues
Closes #123
```

### Review Process

1. **Automated checks**: CI must pass (tests, clippy, formatting)
2. **Code review**: At least one maintainer approval required
3. **Discussion**: Address reviewer feedback
4. **Merge**: Squash and merge to keep history clean

### What Reviewers Look For

- Code quality and style consistency
- Test coverage for new features
- Clear, descriptive commit messages
- Documentation updates (README, code comments)
- No breaking changes without discussion
- Performance considerations for critical paths

## Security Considerations

### Never Commit Secrets

- API keys
- Authentication tokens
- TikTok cookies
- Test URLs that may contain private information

### Configuration Management

- Use environment variables for sensitive data
- Document required environment variables in README
- Add `.env` to `.gitignore`
- Consider using `~/.config/tikd-r/config.toml` for persistent config

### Input Validation

- **Validate all user input**: URLs, file paths, configuration values
- **Sanitize file paths**: Prevent directory traversal attacks
- **Validate URLs**: Ensure they're well-formed before network calls
- **Limit file writes**: Only write to the working directory or subdirectories

### Example

```rust
fn validate_url(url: &str) -> Result<Url> {
    let parsed = Url::parse(url)?;

    // Ensure it's a TikTok URL
    if !parsed.host_str().map_or(false, |h| h.contains("tiktok.com")) {
        return Err(Error::InvalidUrl("Not a TikTok URL".into()));
    }

    Ok(parsed)
}
```

### Dependency Security

- Regularly update dependencies: `cargo update`
- Check for security advisories: `cargo audit` (install via `cargo install cargo-audit`)
- Review dependency changes in pull requests

## Questions or Issues?

- Open an issue: [GitHub Issues](https://github.com/rutthawitc/TikD-R/issues)
- Start a discussion: [GitHub Discussions](https://github.com/rutthawitc/TikD-R/discussions)
- Review existing PRs for examples

## Code of Conduct

- Be respectful and constructive
- Welcome newcomers and help them learn
- Focus on the code, not the person
- Assume good intentions

Thank you for contributing to TikD-R!
