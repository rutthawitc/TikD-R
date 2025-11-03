use std::path::PathBuf;

use clap::Parser;

use crate::error::Error;

/// Command line arguments supported by the TikD-R binary.
#[derive(Debug, Parser)]
#[command(
    name = "tikd-r",
    about = "Download TikTok videos via a fast Rust CLI.",
    version,
    author,
    arg_required_else_help = true
)]
pub struct Cli {
    /// Download a single TikTok video by URL.
    #[arg(value_name = "VIDEO_URL")]
    pub url: Option<String>,

    /// Path to a file with line-delimited TikTok URLs for batch downloads.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Maximum number of concurrent downloads.
    #[arg(long, value_name = "NUM", value_parser = clap::value_parser!(usize))]
    pub max_concurrent: Option<usize>,

    /// Maximum retry attempts per URL on transient failures.
    #[arg(long, value_name = "NUM", value_parser = clap::value_parser!(usize))]
    pub max_retries: Option<usize>,

    /// Initial backoff delay in milliseconds for retry scheduling.
    #[arg(long, value_name = "MILLISECONDS", value_parser = clap::value_parser!(u64))]
    pub backoff_ms: Option<u64>,
}

impl Cli {
    /// Ensure the caller supplies either a single URL or a file path.
    pub fn validate(&self) -> Result<(), Error> {
        match (self.url.as_ref(), self.file.as_ref()) {
            (Some(_), Some(_)) => Err(Error::InputConflict),
            (None, None) => Err(Error::MissingInput),
            _ => Ok(()),
        }
    }
}
