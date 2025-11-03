#![cfg(feature = "live-tests")]

use tikd_r::downloader::build_http_client;
use tikd_r::scraper::Scraper;

/// Fetch a real TikTok share page to ensure parsing still works.
#[tokio::test]
async fn resolves_descriptor_from_live_url() {
    let url = match std::env::var("TIKD_R_LIVE_URL") {
        Ok(value) => value,
        Err(_) => {
            eprintln!("skipping live test; set TIKD_R_LIVE_URL to enable");
            return;
        }
    };

    let client = build_http_client().expect("build http client");
    let scraper = Scraper::new(client);

    let descriptor = scraper
        .extract_video_descriptor(&url)
        .await
        .expect("resolve live descriptor");

    assert!(!descriptor.video_id.is_empty());
    assert!(descriptor.download_url.starts_with("http"));

    if let Ok(expected) = std::env::var("TIKD_R_EXPECT_VIDEO_ID") {
        assert_eq!(descriptor.video_id, expected);
    }
}
