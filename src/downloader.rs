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

        tracing::debug!(
            "Extracted descriptor - video_id: {}, has_download_url: {}, has_play_url: {}",
            descriptor.video_id,
            descriptor.download_url.is_some(),
            descriptor.play_url.is_some()
        );

        let output_path = build_output_path(&descriptor)?;
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let download_url = descriptor.download_url.clone();
        let play_url = descriptor.play_url.clone();

        if let Some(url) = download_url {
            tracing::debug!("Attempting binary download from: {}", url);
            match self.download_binary(&url, share_url, &output_path).await {
                Ok(()) => {
                    tracing::debug!("Binary download succeeded");
                    return Ok(output_path);
                }
                Err(err) => {
                    tracing::warn!("Binary download failed: {}", err);
                    if let Some(ref fallback_url) = play_url {
                        if should_try_hls_fallback(&err) {
                            tracing::info!("Attempting HLS fallback from: {}", fallback_url);
                            self.download_hls_stream(fallback_url, share_url, &output_path)
                                .await?;
                            return Ok(output_path);
                        } else {
                            tracing::warn!("Error not eligible for HLS fallback");
                        }
                    } else {
                        tracing::warn!("No play_url available for HLS fallback");
                    }
                    return Err(err);
                }
            }
        }

        if let Some(url) = play_url {
            tracing::info!("No download_url, attempting HLS stream from: {}", url);
            self.download_hls_stream(&url, share_url, &output_path)
                .await?;
            return Ok(output_path);
        }

        tracing::error!("No download_url or play_url found");
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
        tracing::debug!("Parsing HLS URL: {}", play_url);
        let mut playlist_url =
            Url::parse(play_url).map_err(|_| Error::InvalidUrl(play_url.to_string()))?;

        tracing::debug!("Fetching content from: {}", playlist_url);
        let response = self
            .client
            .get(playlist_url.clone())
            .header(reqwest::header::REFERER, share_url)
            .send()
            .await?
            .error_for_status()?;

        // Check if this is actually a video file, not a playlist
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        tracing::debug!("Content-Type: {}", content_type);

        // If it's a direct video file, download it directly
        if content_type.contains("video/") || content_type.contains("application/octet-stream") {
            tracing::info!("Detected direct video download (not HLS), downloading binary content");
            let mut file = tokio::fs::File::create(output_path).await?;

            let mut total_bytes = 0;
            let mut response = response;
            while let Some(chunk) = response.chunk().await? {
                total_bytes += chunk.len();
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            tracing::info!("Downloaded {} bytes as direct video file", total_bytes);
            return Ok(());
        }

        // Otherwise, treat as HLS playlist
        let mut playlist_body = response.text().await?;
        tracing::debug!("Playlist size: {} bytes", playlist_body.len());

        // Sanity check: ensure it looks like a playlist
        if !playlist_body.trim_start().starts_with("#EXTM3U") {
            tracing::warn!("Content doesn't start with #EXTM3U, may not be valid HLS playlist");
            // Try to detect if it's binary data being misinterpreted
            if playlist_body.starts_with("ftyp") || playlist_body.as_bytes().starts_with(&[0x00, 0x00, 0x00]) {
                tracing::error!("Detected binary video data instead of playlist text");
                return Err(Error::UnsupportedStream(
                    "Received binary video data when expecting HLS playlist".to_string()
                ));
            }
        }

        if is_master_playlist(&playlist_body) {
            tracing::debug!("Detected master playlist, selecting variant");
            let variant_url = select_best_variant(&playlist_body, &playlist_url)
                .ok_or(Error::VideoUrlNotFound)?;
            tracing::debug!("Selected variant: {}", variant_url);

            let response = self
                .client
                .get(variant_url.clone())
                .header(reqwest::header::REFERER, share_url)
                .send()
                .await?
                .error_for_status()?;

            playlist_body = response.text().await?;
            playlist_url = variant_url;
            tracing::debug!("Variant playlist size: {} bytes", playlist_body.len());
        } else {
            tracing::debug!("Processing media playlist directly");
        }

        self.persist_media_playlist(&playlist_body, &playlist_url, share_url, output_path)
            .await
    }

    async fn persist_media_playlist(
        &self,
        playlist_body: &str,
        playlist_url: &Url,
        share_url: &str,
        output_path: &Path,
    ) -> Result<()> {
        tracing::debug!("Creating output file: {:?}", output_path);
        let mut file = tokio::fs::File::create(output_path).await?;
        let mut had_segment = false;
        let mut segment_count = 0;

        tracing::debug!("Processing playlist lines...");
        for (line_num, line) in playlist_body.lines().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("#EXT-X-KEY") {
                let method =
                    extract_attribute(trimmed, "METHOD").unwrap_or_else(|| "NONE".to_string());
                tracing::debug!("Found encryption key: METHOD={}", method);
                if method != "NONE" {
                    return Err(Error::UnsupportedStream(format!(
                        "HLS encryption method {method} is not supported"
                    )));
                }
                continue;
            }

            if trimmed.starts_with("#EXT-X-MAP") {
                if let Some(uri) = extract_attribute(trimmed, "URI") {
                    tracing::debug!("Found initialization segment: {}", uri);
                    let init_url = match resolve_segment_url(playlist_url, &uri) {
                        Ok(url) => url,
                        Err(e) => {
                            tracing::error!("Failed to resolve init segment URL '{}': {}", uri, e);
                            return Err(Error::InvalidUrl(format!("init segment: {}", uri)));
                        }
                    };
                    tracing::debug!("Downloading init segment from: {}", init_url);
                    self.write_segment(&init_url, share_url, &mut file).await?;
                }
                continue;
            }

            if trimmed.starts_with('#') {
                continue;
            }

            // This is a segment URL
            let segment_url = match resolve_segment_url(playlist_url, trimmed) {
                Ok(url) => url,
                Err(e) => {
                    tracing::error!("Failed to resolve segment URL '{}' at line {}: {}", trimmed, line_num + 1, e);
                    return Err(Error::InvalidUrl(format!("segment at line {}: {}", line_num + 1, trimmed)));
                }
            };

            segment_count += 1;
            tracing::debug!("Downloading segment {} from: {}", segment_count, segment_url);
            self.write_segment(&segment_url, share_url, &mut file)
                .await?;
            had_segment = true;
        }

        if !had_segment {
            tracing::error!("No segments found in playlist");
            return Err(Error::VideoUrlNotFound);
        }

        tracing::info!("Downloaded {} segments successfully", segment_count);
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
            tracing::error!("Segment download failed with status: {:?}", err);
            return Err(Error::Network(err));
        }

        let mut bytes_written = 0;
        while let Some(chunk) = response.chunk().await? {
            bytes_written += chunk.len();
            file.write_all(&chunk).await?;
        }

        tracing::debug!("Wrote {} bytes for segment", bytes_written);
        Ok(())
    }
}

/// Resolve a segment URL relative to the playlist URL, or use it as-is if it's absolute.
fn resolve_segment_url(playlist_url: &Url, segment_path: &str) -> Result<Url> {
    // If the segment is already an absolute URL, parse and use it directly
    if segment_path.starts_with("http://") || segment_path.starts_with("https://") {
        return Url::parse(segment_path)
            .map_err(|e| Error::InvalidUrl(format!("absolute URL parse failed: {}", e)));
    }

    // Otherwise, join it with the playlist URL as a relative path
    playlist_url
        .join(segment_path)
        .map_err(|e| Error::InvalidUrl(format!("URL join failed for '{}': {}", segment_path, e)))
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
    let mut lines = playlist.lines().peekable();
    let mut variant_count = 0;

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with("#EXT-X-STREAM-INF") {
            continue;
        }

        variant_count += 1;
        let bandwidth = extract_attribute(trimmed, "BANDWIDTH")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);

        tracing::debug!("Found variant {} with bandwidth: {}", variant_count, bandwidth);

        // Find the next non-empty, non-comment line
        let uri_line = loop {
            match lines.next() {
                Some(line) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        break trimmed;
                    }
                }
                None => {
                    tracing::warn!("No URI found for variant {}", variant_count);
                    return best.map(|(_, url)| url);
                }
            }
        };

        tracing::debug!("Variant {} URI: {}", variant_count, uri_line);

        match resolve_segment_url(base_url, uri_line) {
            Ok(candidate_url) => {
                match &mut best {
                    Some((best_bw, _)) if bandwidth <= *best_bw => {
                        tracing::debug!("Variant {} bandwidth {} <= current best {}, skipping",
                            variant_count, bandwidth, best_bw);
                    }
                    _ => {
                        tracing::debug!("Variant {} is new best with bandwidth {}", variant_count, bandwidth);
                        best = Some((bandwidth, candidate_url));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to resolve variant {} URL '{}': {}", variant_count, uri_line, e);
            }
        }
    }

    if let Some((bw, ref url)) = best {
        tracing::info!("Selected best variant with bandwidth {} from {} variants: {}",
            bw, variant_count, url);
    } else {
        tracing::error!("No valid variants found in master playlist");
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
