use std::path::PathBuf;

use thiserror::Error;

/// Unified error type for the TikD-R application.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Provide either a single TikTok URL or --file, not both.")]
    InputConflict,
    #[error("Provide a TikTok URL or --file with URLs to download.")]
    MissingInput,
    #[error("Invalid TikTok URL: {0}")]
    InvalidUrl(String),
    #[error("No TikTok URLs found in file: {0}")]
    EmptyUrlFile(PathBuf),
    #[error("Unable to locate TikTok video download URL from page.")]
    VideoUrlNotFound,
    #[error("Download summary: {succeeded} succeeded, {failed} failed.")]
    DownloadSummary { succeeded: usize, failed: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Parsing(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
