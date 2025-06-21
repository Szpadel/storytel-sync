#[allow(dead_code)]
type SenderType = ();

#[allow(dead_code)]
type ReceiverType = ();

use std::fs;
use std::io;
use std::path::Path;

pub fn is_downloaded(dst_dir: &Path, author: &str, title: &str) -> bool {
    dst_dir.join(author).join(title).join("audio.mp3").exists()
}

pub fn download_cover(cover_url: &str, book_path: &Path) {
    // pick extension from url, default to jpg
    let ext = Path::new(cover_url)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let target = book_path.join(format!("cover.{ext}"));

    tracing::debug!("download_cover: url={cover_url}, target={:?}", target);

    if target.exists() {
        tracing::debug!("download_cover: target already exists - skip");
        return;
    }

    match reqwest::blocking::get(cover_url) {
        Ok(mut resp) => {
            if resp.status().is_success() {
                match fs::File::create(&target) {
                    Ok(mut file) => match io::copy(&mut resp, &mut file) {
                        Ok(bytes) => tracing::info!("download_cover: wrote {bytes} bytes to {:?}", target),
                        Err(e) => tracing::error!("download_cover: failed writing file - {e}"),
                    },
                    Err(e) => tracing::error!("download_cover: cannot create {:?} - {e}", target),
                }
            } else {
                tracing::warn!("download_cover: request returned {}", resp.status());
            }
        }
        Err(e) => tracing::error!("download_cover: HTTP request failed - {e}"),
    }
}
