use crate::client_storytel_api::{self, ClientData};
use crate::config::Config;
use actix_web::http::header;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use rand::{Rng, rng};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Write;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;

type ProgressStatus = (u64, Option<u64>);
type ProgressMap = HashMap<u64, ProgressStatus>;
type ProgressData = web::Data<Mutex<ProgressMap>>;

fn fmt_bytes(mut bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut idx = 0;
    while bytes >= 1024 && idx < UNITS.len() - 1 {
        bytes /= 1024;
        idx += 1;
    }
    format!("{bytes} {}", UNITS[idx])
}

fn fmt_eta(secs: u64) -> String {
    let h = secs / 3_600;
    let m = (secs % 3_600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

async fn sync_worker(
    client: web::Data<Mutex<ClientData>>,
    dl_dir: web::Data<PathBuf>,
    progress: ProgressData,
) {
    // initial delay - 10 min
    tokio::time::sleep(Duration::from_secs(600)).await;

    loop {
        // run one full sync in a blocking thread
        let client = client.clone();
        let dl_dir = dl_dir.clone();
        let progress = progress.clone();
        tokio::spawn(async move {
            let mut cd = client.lock().await;

            // fresh bookshelf
            let shelf = client_storytel_api::get_bookshelf(&mut cd).await.unwrap();

            let (mut already_synced, mut need_sync) = (0, 0);
            for be in &shelf.books {
                if be.abook.is_some() {
                    let author = be.book.authors_as_string.as_deref().unwrap_or("unknown");
                    let title = &be.book.name;
                    let sanitize = |s: &str| s.replace(['/', '\\'], "_");
                    if crate::download::is_downloaded(&dl_dir, &sanitize(author), &sanitize(title))
                    {
                        already_synced += 1;
                    } else {
                        need_sync += 1;
                    }
                }
            }
            tracing::info!(
                "sync_worker: starting sync pass – already_synced={}, need_sync={}",
                already_synced,
                need_sync
            );

            for be in shelf.books {
                let id = match be.abook {
                    Some(a) => a.id,
                    None => continue,
                };

                // own the pieces we will move into the download closure
                let author: String = be
                    .book
                    .authors_as_string
                    .unwrap_or_else(|| "unknown".into());
                let title: String = be.book.name;
                let sanitize = |s: &str| s.replace(['/', '\\'], "_");
                let author_s = sanitize(&author);
                let title_s = sanitize(&title);

                let cover_rel = be
                    .cover
                    .as_ref()
                    .or(be.book.cover.as_ref())
                    .map_or("/images/nocover.png", String::as_str);
                let cover_url = format!("https://www.storytel.com{cover_rel}");

                if crate::download::is_downloaded(&dl_dir, &author_s, &title_s) {
                    continue; // already there
                }

                tracing::info!("sync_worker: downloading {author}/{title} (id={id})");

                // obtain stream url (needs &mut cd)
                let stream = client_storytel_api::get_stream_url(&mut cd, id)
                    .await
                    .unwrap();
                drop(cd); // release lock during long download

                let target = dl_dir.join(&author_s).join(&title_s);
                let mut last = Instant::now();
                let prog_inner = progress.clone();

                let author_clone = author.clone();
                let title_clone = title.clone();

                client_storytel_api::download_stream_with_progress(
                    &stream,
                    &target,
                    move |done, total| {
                        if let Ok(mut map) = prog_inner.try_lock() {
                            map.insert(id, (done, total));
                        }
                        if last.elapsed().as_secs() >= 60 {
                            last = Instant::now();
                            tracing::info!(
                                "[sync] {author_clone}/{title_clone}  {} / {}",
                                fmt_bytes(done),
                                total.map_or_else(|| "?".into(), fmt_bytes)
                            );
                        }
                    },
                )
                .await
                .unwrap();
                crate::download::download_cover(&cover_url, &target)
                    .await
                    .unwrap();
                tracing::info!("sync_worker: finished {author}/{title}");
                progress.lock().await.remove(&id);

                // reacquire client lock for next book
                cd = client.lock().await;
            }
        });

        // sleep 24 h +/- 2 h jitter
        let base: i32 = 86_400; // 24 h
        let jitter: i32 = rng().random_range(-7_200..=7_200); // +/-2 h
        let delay = u64::try_from(base + jitter).expect("always positive");
        tokio::time::sleep(Duration::from_secs(delay)).await;
    }
}

async fn list(
    data: web::Data<Mutex<ClientData>>,
    download_dir: web::Data<PathBuf>,
    progress: ProgressData,
) -> impl Responder {
    // fetch bookshelf on a blocking thread
    let bookshelf = {
        let mut cd = data.lock().await;
        match client_storytel_api::get_bookshelf(&mut cd).await {
            Ok(bs) => bs,
            Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
        }
    };

    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Bookshelf</title>
<style>
 body {font-family:sans-serif;background:#f4f4f4;margin:0;padding:20px}
 h1   {text-align:center}
 .books{display:flex;flex-wrap:wrap;gap:20px;justify-content:center}
 .card{background:#fff;width:160px;border-radius:4px;
       box-shadow:0 2px 4px rgba(0,0,0,.1);overflow:hidden;display:flex;flex-direction:column}
 .card img{width:100%;height:auto}
 .info{padding:10px;flex:1}
 .title{font-size:14px;font-weight:bold;margin-bottom:6px}
 .author{font-size:12px;color:#666;margin-bottom:6px}
 .isbn{font-size:11px;color:#999}
 .actions{padding:10px;text-align:center}
 button{padding:6px 12px;border:none;border-radius:3px;background:#1976d2;color:#fff;cursor:pointer}
 button[disabled]{background:#aaa;cursor:default}
</style>
</head>
<body>
<h1>Bookshelf</h1>
<div class="books">
"#,
    );

    for book_entry in &bookshelf.books {
        let name = &book_entry.book.name;
        let author = book_entry.book.authors_as_string.as_deref().unwrap_or("");
        let isbn = book_entry
            .isbn
            .as_deref()
            .or(book_entry.book.isbn.as_deref())
            .or(book_entry.abook.as_ref().and_then(|a| a.isbn.as_deref()))
            .unwrap_or("");
        let id = book_entry.abook.as_ref().map(|a| a.id);

        // sanitise for file-system use
        let sanitize = |s: &str| s.replace(['/', '\\'], "_");
        let author_s = sanitize(author);
        let title_s = sanitize(name);

        // cover relative paths come from API – prepend host to make absolute
        let cover_rel = book_entry
            .cover
            .as_ref()
            .or(book_entry.book.cover.as_ref())
            .map_or("/images/nocover.png", String::as_str);
        let cover_url = format!("https://www.storytel.com{cover_rel}");

        let downloading = progress.lock().await.get(&id.unwrap_or(0)).copied();
        let downloaded =
            id.is_some_and(|_| crate::download::is_downloaded(&download_dir, &author_s, &title_s));

        let pct = downloading.map(|(d, t)| t.map_or(0, |tot| 100 * d / tot));
        let btn = if let Some(pct) = pct {
            format!("<button disabled>Downloading {pct}%</button>")
        } else if downloaded {
            "<button disabled>Downloaded</button>".to_owned()
        } else if let Some(book_id) = id {
            format!(
                r#"<form method="post" action="/download/{book_id}">
                    <button type="submit">Download</button>
                   </form>"#
            )
        } else {
            String::new()
        };

        write!(
            &mut html,
            r#"<div class="card">
<img src="{cover_url}" alt="cover">
<div class="info">
  <div class="author">{author}</div>
  <div class="title">{name}</div>
  <div class="isbn">{isbn}</div>
</div>
<div class="actions">{btn}</div>
</div>"#
        )
        .unwrap();
    }
    html.push_str("</div></body></html>");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

async fn download(
    path: web::Path<u64>,
    data: web::Data<Mutex<ClientData>>,
    download_dir: web::Data<PathBuf>,
    progress: ProgressData,
) -> impl Responder {
    let id = path.into_inner();

    //  kick off a background blocking task; reply immediately
    let progress_bg = progress.clone();
    let download_dir = download_dir.get_ref().clone();
    let data = data.clone();
    tokio::spawn(async move {
        // ---- quick section: read API, release lock ----
        let (name, author, stream_url, cover_url) = {
            let mut cd = data.lock().await;

            let bookshelf = client_storytel_api::get_bookshelf(&mut cd).await.unwrap();
            let (name, author) = bookshelf
                .books
                .iter()
                .find_map(|b| {
                    b.abook.as_ref().filter(|a| a.id == id).map(|_| {
                        (
                            b.book.name.clone(),
                            b.book
                                .authors_as_string
                                .clone()
                                .unwrap_or_else(|| "unknown".into()),
                        )
                    })
                })
                .unwrap_or_else(|| (format!("book_{id}"), "unknown".into()));

            let cover_rel = bookshelf
                .books
                .iter()
                .find_map(|b| {
                    b.abook
                        .as_ref()
                        .filter(|a| a.id == id)
                        .and_then(|_| b.cover.as_ref().or(b.book.cover.as_ref()).cloned())
                })
                .unwrap_or_else(|| "/images/nocover.png".into());

            let url = client_storytel_api::get_stream_url(&mut cd, id)
                .await
                .unwrap();

            (
                name,
                author,
                url,
                format!("https://www.storytel.com{cover_rel}"),
            )
        };

        tracing::info!("download: starting {author}/{name} (id={id})");

        let sanitize = |s: &str| s.replace(['/', '\\'], "_");
        let author_s = sanitize(&author);
        let title_s = sanitize(&name);

        let name_clone = name.clone();

        if crate::download::is_downloaded(&download_dir, &author_s, &title_s) {
            return;
        }

        let target = download_dir.join(&author_s).join(&title_s);
        let target_clone = target.clone();
        tracing::debug!(
            "download: id={id}, target={:?}, cover_url={}",
            target_clone,
            cover_url
        );

        let progress_inner = progress_bg.clone(); // move into closure

        let mut last_print = Instant::now();
        client_storytel_api::download_stream_with_progress(
            &stream_url,
            &target_clone,
            move |done, total| {
                // UI progress
                if let Ok(mut map) = progress_inner.try_lock() {
                    map.insert(id, (done, total));
                }

                // console stats every minute
                if last_print.elapsed().as_secs() >= 60 {
                    last_print = Instant::now();

                    let speed = done / 60;
                    let eta = total.and_then(|t| {
                        if speed != 0 {
                            Some((t - done) / speed)
                        } else {
                            None
                        }
                    });

                    tracing::info!(
                        "[{name_clone}] {} / {} @ {}/s {}",
                        fmt_bytes(done),
                        total.map_or_else(|| "?".into(), fmt_bytes),
                        fmt_bytes(speed),
                        eta.map(|s| format!("ETA {}", fmt_eta(s)))
                            .unwrap_or_default()
                    );
                }
            },
        )
        .await
        .unwrap();

        tracing::debug!("download: audio done, downloading cover {}", cover_url);
        crate::download::download_cover(&cover_url, &target_clone)
            .await
            .unwrap();

        tracing::info!("download: finished {author}/{name}");

        // finished - remove entry
        progress_bg.lock().await.remove(&id);
    });

    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, "/"))
        .finish()
}

pub async fn run(client: ClientData, cfg: &Config, host: &str, port: u16) {
    let download_dir = cfg.download_dir.clone();
    let client_data = web::Data::new(Mutex::new(client));
    let download_dir_data = web::Data::new(download_dir.clone());
    let progress: ProgressData = web::Data::new(Mutex::new(HashMap::new()));

    if cfg.sync_enabled {
        tokio::spawn(sync_worker(
            client_data.clone(),
            download_dir_data.clone(),
            progress.clone(),
        ));
    }

    HttpServer::new(move || {
        App::new()
            .app_data(client_data.clone())
            .app_data(download_dir_data.clone())
            .app_data(progress.clone())
            .route("/", web::get().to(list))
            .route("/download/{id}", web::post().to(download))
    })
    .bind((host, port))
    .expect("bind failed")
    .run()
    .await
    .expect("server failed");
}
