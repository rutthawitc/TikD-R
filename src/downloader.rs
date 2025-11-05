use std::path::{Path, PathBuf};
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
use url::Url;

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

        let download_url = descriptor.download_url.clone();
        let play_url = descriptor.play_url.clone();

        if let Some(url) = download_url {
            match self.download_binary(&url, share_url, &output_path).await {
                Ok(()) => return Ok(output_path),
                Err(err) => {
                    if let Some(ref fallback_url) = play_url {
                        if should_try_hls_fallback(&err) {
                            self.download_hls_stream(fallback_url, share_url, &output_path)
                                .await?;
                            return Ok(output_path);
                        }
                    }
                    return Err(err);
                }
            }
        }

        if let Some(url) = play_url {
            self.download_hls_stream(&url, share_url, &output_path)
                .await?;
            return Ok(output_path);
        }

        Err(Error::VideoUrlNotFound)
    }

    async fn download_binary(&self, url: &str, share_url: &str, output_path: &Path) -> Result<()> {
        let mut response = self
            .client
            .get(url)
            .header(reqwest::header::REFERER, share_url)
            .send()
            .await?;

        if let Err(err) = response.error_for_status_ref() {
            return Err(Error::Network(err));
        }

        let mut file = tokio::fs::File::create(output_path).await?;

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        Ok(())
    }

    async fn download_hls_stream(
        &self,
        play_url: &str,
        share_url: &str,
        output_path: &Path,
    ) -> Result<()> {
        let mut playlist_url =
            Url::parse(play_url).map_err(|_| Error::InvalidUrl(play_url.to_string()))?;

        let mut playlist_body = self.fetch_playlist(&playlist_url, share_url).await?;

        if is_master_playlist(&playlist_body) {
            let variant_url = select_best_variant(&playlist_body, &playlist_url)
                .ok_or(Error::VideoUrlNotFound)?;
            playlist_body = self.fetch_playlist(&variant_url, share_url).await?;
            playlist_url = variant_url;
        }

        self.persist_media_playlist(&playlist_body, &playlist_url, share_url, output_path)
            .await
    }

    async fn fetch_playlist(&self, url: &Url, share_url: &str) -> Result<String> {
        let response = self
            .client
            .get(url.clone())
            .header(reqwest::header::REFERER, share_url)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.text().await?)
    }

    async fn persist_media_playlist(
        &self,
        playlist_body: &str,
        playlist_url: &Url,
        share_url: &str,
        output_path: &Path,
    ) -> Result<()> {
        let mut file = tokio::fs::File::create(output_path).await?;
        let mut had_segment = false;

        for line in playlist_body.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("#EXT-X-KEY") {
                let method =
                    extract_attribute(trimmed, "METHOD").unwrap_or_else(|| "NONE".to_string());
                if method != "NONE" {
                    return Err(Error::UnsupportedStream(format!(
                        "HLS encryption method {method} is not supported"
                    )));
                }
                continue;
            }

            if trimmed.starts_with("#EXT-X-MAP") {
                if let Some(uri) = extract_attribute(trimmed, "URI") {
                    let init_url = playlist_url
                        .join(&uri)
                        .map_err(|_| Error::InvalidUrl(uri.clone()))?;
                    self.write_segment(&init_url, share_url, &mut file).await?;
                }
                continue;
            }

            if trimmed.starts_with('#') {
                continue;
            }

            let segment_url = playlist_url
                .join(trimmed)
                .map_err(|_| Error::InvalidUrl(trimmed.to_string()))?;
            self.write_segment(&segment_url, share_url, &mut file)
                .await?;
            had_segment = true;
        }

        if !had_segment {
            return Err(Error::VideoUrlNotFound);
        }

        file.flush().await?;
        Ok(())
    }

    async fn write_segment(
        &self,
        segment_url: &Url,
        share_url: &str,
        file: &mut tokio::fs::File,
    ) -> Result<()> {
        let mut response = self
            .client
            .get(segment_url.clone())
            .header(reqwest::header::REFERER, share_url)
            .send()
            .await?;

        if let Err(err) = response.error_for_status_ref() {
            return Err(Error::Network(err));
        }

        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }

        Ok(())
    }
}

fn should_try_hls_fallback(err: &Error) -> bool {
    match err {
        Error::Network(inner) => {
            inner.status().map_or(false, |status| {
                matches!(
                    status,
                    StatusCode::FORBIDDEN
                        | StatusCode::UNAUTHORIZED
                        | StatusCode::NOT_FOUND
                        | StatusCode::GONE
                        | StatusCode::LOCKED
                        | StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
                )
            }) || inner.is_builder()
        }
        _ => false,
    }
}

fn is_master_playlist(playlist: &str) -> bool {
    playlist
        .lines()
        .any(|line| line.trim_start().starts_with("#EXT-X-STREAM-INF"))
}

fn select_best_variant(playlist: &str, base_url: &Url) -> Option<Url> {
    let mut best: Option<(u64, Url)> = None;
    let mut lines = playlist.lines();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with("#EXT-X-STREAM-INF") {
            continue;
        }

        let bandwidth = extract_attribute(trimmed, "BANDWIDTH")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        let uri_line = lines.next()?.trim();
        if uri_line.is_empty() {
            continue;
        }

        if let Ok(candidate_url) = base_url.join(uri_line) {
            match &mut best {
                Some((best_bw, _)) if bandwidth <= *best_bw => {}
                _ => best = Some((bandwidth, candidate_url)),
            }
        }
    }

    best.map(|(_, url)| url)
}

fn extract_attribute(line: &str, attribute: &str) -> Option<String> {
    let needle = format!("{attribute}=");
    let start = line.find(&needle)? + needle.len();
    let remainder = &line[start..];

    if remainder.starts_with('"') {
        let remainder = &remainder[1..];
        let end = remainder.find('"')?;
        Some(remainder[..end].to_string())
    } else {
        let end = remainder
            .find(',')
            .or_else(|| remainder.find(' '))
            .unwrap_or(remainder.len());
        Some(remainder[..end].to_string())
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
        Error::UnsupportedStream(_) => false,
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
            download_url: Some("https://example.com".into()),
            play_url: None,
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

    #[test]
    fn extract_attribute_parses_quoted_values() {
        let line = r#"#EXT-X-MAP:URI="init.mp4""#;
        assert_eq!(extract_attribute(line, "URI"), Some("init.mp4".to_string()));
    }

    #[test]
    fn extract_attribute_parses_unquoted_values() {
        let line = "#EXT-X-KEY:METHOD=AES-128,URI=\"enc.key\"";
        assert_eq!(
            extract_attribute(line, "METHOD"),
            Some("AES-128".to_string())
        );
    }
}
