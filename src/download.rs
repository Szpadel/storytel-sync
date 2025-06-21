#[allow(dead_code)]
type SenderType = ();

#[allow(dead_code)]
type ReceiverType = ();

use tokio::{fs, io::AsyncWriteExt};
use futures_util::StreamExt;
use std::path::Path;

pub fn is_downloaded(dst_dir: &Path, author: &str, title: &str) -> bool {
    dst_dir.join(author).join(title).join("audio.mp3").exists()
}

pub async fn download_cover(cover_url: &str, book_path: &Path) -> eyre::Result<()> {

    let ext = Path::new(cover_url)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let target = book_path.join(format!("cover.{ext}"));

    if target.exists() {
        return Ok(());
    }

    let resp = reqwest::get(cover_url).await?;
    if !resp.status().is_success() {
        tracing::warn!("download_cover: request returned {}", resp.status());
        return Ok(());
    }

    fs::create_dir_all(book_path).await?;
    let mut file = fs::File::create(&target).await?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    Ok(())
}
