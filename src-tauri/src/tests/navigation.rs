use tauri::Url;

use crate::navigation::is_app_url;

fn url(text: &str) -> Url {
    Url::parse(text).expect("a valid URL")
}

fn dev() -> Url {
    url("http://localhost:1420")
}

#[test]
fn the_bundled_app_is_allowed() {
    for page in [
        "tauri://localhost",
        "tauri://localhost/",
        "tauri://localhost/index.html",
        "tauri://localhost/library?course=1#top",
    ] {
        assert!(is_app_url(&url(page), None), "{page} is the app");
        assert!(is_app_url(&url(page), Some(&dev())), "{page} is the app");
    }
}

#[test]
fn the_development_server_is_allowed_only_when_it_serves_the_app() {
    for page in ["http://localhost:1420/", "http://localhost:1420/reader?x=1"] {
        assert!(is_app_url(&url(page), Some(&dev())), "{page} is the app");
        assert!(
            !is_app_url(&url(page), None),
            "{page} is not the app without a development server"
        );
    }
}

#[test]
fn every_other_url_is_refused() {
    for page in [
        "https://example.com/",
        "https://chatgpt.com/",
        "https://auth.openai.com/api/accounts/authorize",
        "http://localhost:1421/",
        "https://localhost:1420/",
        "http://127.0.0.1:1420/",
        "http://localhost/",
        "tauri://evil.example/",
        "tauri://localhost.evil.example/",
        "http://tauri.localhost.evil.example/",
        "file:///etc/passwd",
        "about:blank",
        "data:text/html,<script>alert(1)</script>",
        "javascript:alert(1)",
        "blob:tauri://localhost/123",
    ] {
        assert!(
            !is_app_url(&url(page), Some(&dev())),
            "{page} is not the app"
        );
    }
}
