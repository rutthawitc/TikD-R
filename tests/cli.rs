use std::path::PathBuf;

use tikd_r::cli::Cli;

#[test]
fn cli_requires_either_url_or_file() {
    let cli = Cli {
        url: None,
        file: None,
        max_concurrent: None,
        max_retries: None,
        backoff_ms: None,
    };

    assert!(cli.validate().is_err());
}

#[test]
fn cli_rejects_conflicting_inputs() {
    let cli = Cli {
        url: Some("https://www.tiktok.com/@user/video/123".into()),
        file: Some(PathBuf::from("urls.txt")),
        max_concurrent: None,
        max_retries: None,
        backoff_ms: None,
    };

    assert!(cli.validate().is_err());
}

#[test]
fn cli_accepts_single_url() {
    let cli = Cli {
        url: Some("https://www.tiktok.com/@user/video/123".into()),
        file: None,
        max_concurrent: None,
        max_retries: None,
        backoff_ms: None,
    };

    assert!(cli.validate().is_ok());
}
