#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! reqwest = { version = "0.12", features = ["json"] }
//! tokio = { version = "1", features = ["full"] }
//! serde_json = "1"
//! ```

async fn fetch_pypi_readme(package: &str) -> Option<String> {
    let url = format!("https://pypi.org/pypi/{}/json", package);
    let resp: serde_json::Value = reqwest::get(&url).await.ok()?.json().await.ok()?;
    resp["info"]["description"].as_str().map(String::from)
}

#[tokio::main]
async fn main() {
    match fetch_pypi_readme("ruff").await {
        Some(desc) => println!("{}", desc),
        None => println!("failed"),
    }
}
