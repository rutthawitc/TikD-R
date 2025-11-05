use std::collections::HashMap;

use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::error::{Error, Result};

/// Information needed to perform the actual media download.
#[derive(Debug, Clone)]
pub struct VideoDescriptor {
    pub video_id: String,
    pub download_url: Option<String>,
    pub play_url: Option<String>,
    pub author: String,
}

/// Extracts direct video URLs from TikTok share links.
#[derive(Clone)]
pub struct Scraper {
    client: Client,
}

impl Scraper {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch and resolve the downloadable media URL for a TikTok share link.
    pub async fn extract_video_descriptor(&self, share_url: &str) -> Result<VideoDescriptor> {
        if !share_url.contains("tiktok.com") {
            return Err(Error::InvalidUrl(share_url.to_string()));
        }

        let response = self
            .client
            .get(share_url)
            .send()
            .await?
            .error_for_status()?;
        let html = response.text().await?;

        parse_share_page(&html, share_url).ok_or(Error::VideoUrlNotFound)
    }
}

fn parse_share_page(html: &str, share_url: &str) -> Option<VideoDescriptor> {
    let document = Html::parse_document(html);

    parse_universal_data(&document, share_url)
        .or_else(|| parse_sigi_state(&document, share_url))
        .or_else(|| parse_next_data(&document, share_url))
        .map(|mut descriptor| {
            if let Some(ref mut url) = descriptor.download_url {
                *url = url.replace("\\u0026", "&");
            }
            if let Some(ref mut url) = descriptor.play_url {
                *url = url.replace("\\u0026", "&");
            }
            descriptor
        })
}

fn parse_universal_data(document: &Html, share_url: &str) -> Option<VideoDescriptor> {
    let selector = Selector::parse("script#__UNIVERSAL_DATA_FOR_REHYDRATION__").ok()?;
    let element = document.select(&selector).next()?;
    let raw_json = element.text().collect::<String>();
    let value: Value = serde_json::from_str(&raw_json).ok()?;

    let item = value
        .get("__DEFAULT_SCOPE__")
        .and_then(|scope| scope.get("webapp.video-detail"))
        .and_then(|detail| detail.get("itemInfo"))
        .and_then(|info| info.get("itemStruct"))?;

    build_descriptor_from_value(item, share_url)
}

fn parse_sigi_state(document: &Html, share_url: &str) -> Option<VideoDescriptor> {
    let selector = Selector::parse("script#SIGI_STATE").ok()?;
    let element = document.select(&selector).next()?;
    let raw_json = element.text().collect::<String>();
    let sigi_state: SigiState = serde_json::from_str(&raw_json).ok()?;

    resolve_descriptor_from_items(sigi_state.item_module, share_url)
}

fn parse_next_data(document: &Html, share_url: &str) -> Option<VideoDescriptor> {
    let selector = Selector::parse("script#__NEXT_DATA__").ok()?;
    let element = document.select(&selector).next()?;
    let raw_json = element.text().collect::<String>();
    let next_data: NextData = serde_json::from_str(&raw_json).ok()?;

    let items = next_data
        .props
        .page_props
        .item_info
        .item_struct
        .map(|item| {
            let mut map = HashMap::new();
            if let Some(id) = item.id.clone() {
                map.insert(id, item);
            }
            map
        })
        .unwrap_or_default();

    resolve_descriptor_from_items(items, share_url)
}

fn resolve_descriptor_from_items(
    mut items: HashMap<String, ItemStruct>,
    share_url: &str,
) -> Option<VideoDescriptor> {
    if items.is_empty() {
        return None;
    }

    if let Some(video_id) = guess_video_id(share_url) {
        if let Some(item) = items.remove(&video_id) {
            return build_descriptor_from_item(item, share_url);
        }
    }

    // fallback to first entry
    let (_, item) = items.into_iter().next()?;
    build_descriptor_from_item(item, share_url)
}

fn build_descriptor_from_value(value: &Value, share_url: &str) -> Option<VideoDescriptor> {
    let video_id = value
        .get("id")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| guess_video_id(share_url))?;

    let video = value.get("video")?;

    let download_url = video
        .get("downloadAddr")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let play_url = video
        .get("playAddr")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    if download_url.is_none() && play_url.is_none() {
        return None;
    }

    let author = value
        .get("author")
        .and_then(|auth| auth.get("uniqueId"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| guess_author_id(share_url))
        .unwrap_or_else(|| "unknown".to_string());

    Some(VideoDescriptor {
        video_id,
        download_url,
        play_url,
        author,
    })
}

fn build_descriptor_from_item(item: ItemStruct, share_url: &str) -> Option<VideoDescriptor> {
    let video = item.video?;

    let download_url = video.download_addr.filter(|s| !s.is_empty());

    let play_url = video.play_addr.filter(|s| !s.is_empty());

    if download_url.is_none() && play_url.is_none() {
        return None;
    }

    let author = item
        .author
        .and_then(|a| a.unique_id)
        .or_else(|| guess_author_id(share_url))
        .unwrap_or_else(|| "unknown".to_string());

    Some(VideoDescriptor {
        video_id: item.id?,
        download_url,
        play_url,
        author,
    })
}

fn guess_video_id(share_url: &str) -> Option<String> {
    let url = Url::parse(share_url).ok()?;
    let segments: Vec<_> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();

    for window in segments.windows(2) {
        if let [prefix, id] = window {
            if *prefix == "video" || *prefix == "note" {
                return Some((*id).to_string());
            }
        }
    }

    segments.last().map(|value| (*value).to_string())
}

fn guess_author_id(share_url: &str) -> Option<String> {
    let url = Url::parse(share_url).ok()?;
    for segment in url.path_segments()? {
        if let Some(stripped) = segment.strip_prefix('@') {
            if !stripped.is_empty() {
                return Some(stripped.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct SigiState {
    #[serde(rename = "ItemModule", default)]
    item_module: HashMap<String, ItemStruct>,
}

#[derive(Debug, Deserialize)]
struct NextData {
    props: Props,
}

#[derive(Debug, Deserialize)]
struct Props {
    #[serde(rename = "pageProps")]
    page_props: PageProps,
}

#[derive(Debug, Deserialize)]
struct PageProps {
    #[serde(rename = "itemInfo")]
    item_info: ItemInfo,
}

#[derive(Debug, Deserialize)]
struct ItemInfo {
    #[serde(rename = "itemStruct")]
    item_struct: Option<ItemStruct>,
}

#[derive(Debug, Deserialize, Clone)]
struct ItemStruct {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    video: Option<VideoStruct>,
    #[serde(default)]
    author: Option<AuthorStruct>,
}

#[derive(Debug, Deserialize, Clone)]
struct VideoStruct {
    #[serde(rename = "downloadAddr", default)]
    download_addr: Option<String>,
    #[serde(rename = "playAddr", default)]
    play_addr: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct AuthorStruct {
    #[serde(rename = "uniqueId", default)]
    unique_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_descriptor_from_sigi_state() {
        let html = include_str!("../tests/fixtures/sample_sigi_state.html");
        let document = Html::parse_document(html);
        let descriptor =
            parse_sigi_state(&document, "https://www.tiktok.com/@user/video/1234567890");
        assert!(descriptor.is_some());
        let descriptor = descriptor.unwrap();
        assert_eq!(descriptor.video_id, "1234567890");
        assert!(descriptor
            .download_url
            .as_deref()
            .unwrap()
            .contains("example.com"));
        assert_eq!(descriptor.author, "sigi_author");
    }

    #[test]
    fn parse_descriptor_from_universal_data() {
        let html = include_str!("../tests/fixtures/sample_universal_data.html");
        let document = Html::parse_document(html);
        let descriptor =
            parse_universal_data(&document, "https://www.tiktok.com/@user/video/9876543210");
        assert!(descriptor.is_some());
        let descriptor = descriptor.unwrap();
        assert_eq!(descriptor.video_id, "9876543210");
        assert!(descriptor
            .download_url
            .as_deref()
            .unwrap()
            .contains("example.com"));
        assert_eq!(descriptor.author, "sample_author");
    }

    #[test]
    fn guess_id_handles_numeric_path() {
        let id = guess_video_id("https://www.tiktok.com/@user/video/987654321");
        assert_eq!(id, Some("987654321".into()));
    }

    #[test]
    fn guess_id_falls_back_to_last_segment() {
        let id = guess_video_id("https://www.tiktok.com/t/ZT8abcd/");
        assert_eq!(id, Some("ZT8abcd".into()));
    }
}
