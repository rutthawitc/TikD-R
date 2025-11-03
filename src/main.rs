use std::fs;

use clap::Parser;

use tikd_r::cli::Cli;
use tikd_r::downloader::{DownloadConfig, Downloader};
use tikd_r::error::{Error, Result};

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let cli = Cli::parse();
    cli.validate()?;

    let urls = gather_urls(&cli)?;
    let mut config = DownloadConfig::default();
    if let Some(max) = cli.max_concurrent {
        config.max_concurrent_downloads = max.max(1);
    }
    if let Some(retries) = cli.max_retries {
        config.max_retries = retries;
    }
    if let Some(backoff) = cli.backoff_ms {
        config.initial_backoff_ms = backoff.max(1);
    }

    let downloader = Downloader::with_config(config)?;

    let reports = downloader.download_all(&urls).await;

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for report in &reports {
        match &report.result {
            Ok(path) => {
                succeeded += 1;
                println!("Downloaded {} -> {}", report.url, path.display());
            }
            Err(err) => {
                failed += 1;
                eprintln!("Failed {}: {err}", report.url);
            }
        }
    }

    println!("Summary: {succeeded} succeeded, {failed} failed.");

    if failed > 0 {
        return Err(Error::DownloadSummary { succeeded, failed });
    }

    Ok(())
}

fn gather_urls(cli: &Cli) -> Result<Vec<String>> {
    if let Some(url) = cli.url.as_ref() {
        return Ok(vec![url.trim().to_string()]);
    }

    if let Some(path) = cli.file.as_ref() {
        let contents = fs::read_to_string(path)?;
        let mut urls: Vec<String> = contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(ToOwned::to_owned)
            .collect();

        if urls.is_empty() {
            return Err(Error::EmptyUrlFile(path.clone()));
        }

        urls.dedup();
        return Ok(urls);
    }

    Err(Error::MissingInput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_single_url() {
        let cli = Cli {
            url: Some("https://www.tiktok.com/@user/video/1".into()),
            file: None,
            max_concurrent: None,
            max_retries: None,
            backoff_ms: None,
        };

        let urls = gather_urls(&cli).unwrap();
        assert_eq!(urls, vec!["https://www.tiktok.com/@user/video/1"]);
    }

    #[test]
    fn gather_urls_from_file() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), "https://a\nhttps://b\n").unwrap();

        let cli = Cli {
            url: None,
            file: Some(temp.path().to_path_buf()),
            max_concurrent: None,
            max_retries: None,
            backoff_ms: None,
        };

        let urls = gather_urls(&cli).unwrap();
        assert_eq!(urls, vec!["https://a", "https://b"]);
    }

    #[test]
    fn deduplicate_urls_from_file() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), "https://a\nhttps://a\n").unwrap();

        let cli = Cli {
            url: None,
            file: Some(temp.path().to_path_buf()),
            max_concurrent: None,
            max_retries: None,
            backoff_ms: None,
        };

        let urls = gather_urls(&cli).unwrap();
        assert_eq!(urls, vec!["https://a"]);
    }

    #[test]
    fn empty_file_is_error() {
        let temp = tempfile::NamedTempFile::new().unwrap();

        let cli = Cli {
            url: None,
            file: Some(temp.path().to_path_buf()),
            max_concurrent: None,
            max_retries: None,
            backoff_ms: None,
        };

        let err = gather_urls(&cli).unwrap_err();
        matches!(err, Error::EmptyUrlFile(_));
    }
}
