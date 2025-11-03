use std::path::PathBuf;
use std::sync::Arc;

use futures::stream::{self, StreamExt};
use reqwest::{redirect::Policy, Client, StatusCode};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use tokio::{
    io::AsyncWriteExt,
    time::{sleep, Duration},
};

use crate::error::{Error, Result};
use crate::scraper::{Scraper, VideoDescriptor};

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/124.0.0.0 Safari/537.36";

#[derive(Clone, Debug)]
pub struct DownloadConfig {
    pub max_retries: usize,
    pub initial_backoff_ms: u64,
    pub max_concurrent_downloads: usize,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 500,
            max_concurrent_downloads: 4,
        }
    }
}

/// Expose a configured HTTP client shared by the downloader and integration tests.
pub fn build_http_client() -> Result<Client> {
    let cookie_store = CookieStore::default();
    let cookie_store = Arc::new(CookieStoreMutex::new(cookie_store));

    let client = Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .redirect(Policy::limited(10))
        .cookie_provider(cookie_store)
        .build()?;

    Ok(client)
}

/// Detailed download outcome for reporting and summaries.
#[derive(Debug)]
pub struct DownloadReport {
    pub url: String,
    pub result: Result<PathBuf>,
}

impl DownloadReport {
    fn success(url: String, path: PathBuf) -> Self {
        Self {
            url,
            result: Ok(path),
        }
    }

    fn failure(url: String, err: Error) -> Self {
        Self {
            url,
            result: Err(err),
        }
    }

    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.result.as_ref().ok()
    }

    pub fn error(&self) -> Option<&Error> {
        self.result.as_ref().err()
    }
}

/// High-level orchestrator for downloading one or many TikTok videos.
#[derive(Clone)]
pub struct Downloader {
    client: Client,
    scraper: Scraper,
    config: DownloadConfig,
}

impl Downloader {
    /// Build a downloader with sane defaults for TikTok endpoints.
    pub fn new() -> Result<Self> {
        let client = build_http_client()?;
        Ok(Self::with_client_and_config(
            client,
            DownloadConfig::default(),
        ))
    }

    /// Construct a downloader from a pre-configured HTTP client.
    pub fn with_client(client: Client) -> Self {
        Self::with_client_and_config(client, DownloadConfig::default())
    }

    pub fn with_config(config: DownloadConfig) -> Result<Self> {
        let client = build_http_client()?;
        Ok(Self::with_client_and_config(client, config))
    }

    pub fn with_client_and_config(client: Client, config: DownloadConfig) -> Self {
        let scraper = Scraper::new(client.clone());
        Self {
            client,
            scraper,
            config,
        }
    }

    /// Download all share URLs, returning per-URL outcomes.
    pub async fn download_all(&self, urls: &[String]) -> Vec<DownloadReport> {
        if urls.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<(usize, DownloadReport)> = Vec::with_capacity(urls.len());

        let tasks = stream::iter(urls.iter().cloned().enumerate().map(|(idx, url)| {
            let downloader = self.clone();
            async move {
                let outcome = downloader.download_one(&url).await;
                let report = match outcome {
                    Ok(path) => DownloadReport::success(url, path),
                    Err(err) => DownloadReport::failure(url, err),
                };
                (idx, report)
            }
        }))
        .buffer_unordered(self.config.max_concurrent_downloads);

        futures::pin_mut!(tasks);
        while let Some((idx, report)) = tasks.next().await {
            results.push((idx, report));
        }

        results.sort_by_key(|(idx, _)| *idx);
        results.into_iter().map(|(_, report)| report).collect()
    }

    /// Download a single TikTok share URL to disk and return the output path.
    pub async fn download_one(&self, share_url: &str) -> Result<PathBuf> {
        let mut attempt = 0;

        loop {
            match self.download_once(share_url).await {
                Ok(path) => return Ok(path),
                Err(err) => {
                    attempt += 1;
                    if attempt > self.config.max_retries || !should_retry(&err) {
                        return Err(err);
                    }

                    let backoff_ms = self
                        .config
                        .initial_backoff_ms
                        .saturating_mul(1u64 << (attempt.saturating_sub(1)));
                    sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }
    }

    async fn download_once(&self, share_url: &str) -> Result<PathBuf> {
        let descriptor = self.scraper.extract_video_descriptor(share_url).await?;

        let output_path = build_output_path(&descriptor)?;
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut response = self
            .client
            .get(&descriptor.download_url)
            .header(reqwest::header::REFERER, share_url)
            .send()
            .await?
            .error_for_status()?;

        let mut file = tokio::fs::File::create(&output_path).await?;

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        Ok(output_path)
    }
}

fn build_output_path(descriptor: &VideoDescriptor) -> Result<PathBuf> {
    let video = sanitize_component(&descriptor.video_id);
    if video.is_empty() {
        return Err(Error::InvalidUrl("missing video id".into()));
    }

    let author = sanitize_component(&descriptor.author);
    let author_dir = if author.is_empty() {
        "unknown".to_string()
    } else {
        author
    };

    Ok(PathBuf::from(author_dir).join(format!("{video}.mp4")))
}

fn sanitize_component(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect()
}

fn should_retry(err: &Error) -> bool {
    match err {
        Error::Network(inner) => {
            if inner.is_timeout() || inner.is_connect() || inner.is_body() {
                return true;
            }

            if let Some(status) = inner.status() {
                return status == StatusCode::TOO_MANY_REQUESTS
                    || status == StatusCode::FORBIDDEN
                    || status.is_server_error();
            }

            true
        }
        Error::Io(_) => true,
        Error::Parsing(_) => true,
        Error::InvalidUrl(_) => false,
        Error::InputConflict => false,
        Error::MissingInput => false,
        Error::EmptyUrlFile(_) => false,
        Error::VideoUrlNotFound => false,
        Error::DownloadSummary { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn sanitize_preserves_alphanumeric() {
        let id = "abc123-_./!@";
        assert_eq!(sanitize_component(id), "abc123-_.");
    }

    #[test]
    fn build_output_path_sanitizes_components() {
        let descriptor = VideoDescriptor {
            video_id: "video!@#".into(),
            download_url: "https://example.com".into(),
            author: "@user name".into(),
        };

        let path = build_output_path(&descriptor).unwrap();
        assert_eq!(path, PathBuf::from("username/video.mp4"));
    }

    #[test]
    fn download_all_accumulates_errors() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = build_http_client().unwrap();
            let downloader = Downloader::with_client(client);

            let urls = vec![
                "not-a-tiktok-url".to_string(),
                "https://example.com/".to_string(),
            ];

            let reports = downloader.download_all(&urls).await;
            assert_eq!(reports.len(), 2);
            assert!(reports.iter().all(|r| r.result.is_err()));
        });
    }
}
