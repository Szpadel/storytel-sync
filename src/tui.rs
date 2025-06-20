use cursive::traits::*;
use cursive::views::{Button, Dialog, EditView, LinearLayout, SelectView, TextView};
use cursive::Cursive;
use std::time::Instant;

use crate::{client_storytel_api, mpv, credentials};

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} {}", UNITS[idx])
    } else {
        format!("{value:.1} {}", UNITS[idx])
    }
}

fn format_eta(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn quit_app(siv: &mut Cursive) {
    siv.quit();
}

fn show_player(siv: &mut Cursive, book_mpv: &(String, u64, i64, u64)) {
    let book_name = &book_mpv.0;
    let abook_id  =  book_mpv.1;
    let position  =  book_mpv.2;
    let bookmark_id = book_mpv.3;

    let client_data = siv.user_data::<client_storytel_api::ClientData>().unwrap();
    client_data.current_book_name = Some(book_name.clone());
    client_data.current_abook_id  = Some(abook_id);
    client_data.current_abookmark_id = Some(bookmark_id);

    let url_ask_stream =
        client_storytel_api::get_stream_url(client_data, &abook_id);

    let resp = client_data.request_client.get(&url_ask_stream).send();

    let location = resp.as_ref().unwrap().url().to_owned().to_string();

    let mut seconds: i64 = 0;
    if position != -1 {
        let microsec_to_sec = 1000000;
        seconds = position / microsec_to_sec;
    }

    let (sender, receiver) = mpv::simple_example(location, seconds);
    client_data.sender = Some(sender);
    client_data.receiver = Some(receiver);

    siv.pop_layer();
    siv.add_layer(
        Dialog::around(
            LinearLayout::horizontal()
                .child(Button::new("Play", mpv::play))
                .child(Button::new("Pause", mpv::pause))
                .child(Button::new("Backward", mpv::backward))
                .child(Button::new("Forward", mpv::forward))
                .child(Button::new("Download", download_current))
                .child(Button::new("Exit", show_bookshelf)),
        )
        .title("Player"),
    );
}

pub fn auto_login(siv: &mut Cursive, email: &str, pass: &str) {
    show_check_login(siv, email, pass);
}

fn show_bookshelf(siv: &mut Cursive) {
    let bookshelf = client_storytel_api::get_bookshelf(
        siv.user_data::<client_storytel_api::ClientData>().unwrap(),
    );
    siv.pop_layer();
    let mut book_select: Vec<(String, (String, u64, i64, u64))> = Vec::new();
    for book_entry in bookshelf.books.iter() {
        match &book_entry.abook {
            Some(abook) => book_select.push((
                book_entry.book.name.clone(),
                (
                    book_entry.book.name.clone(), // name
                    abook.id,                     // abook id
                    book_entry.abookmark.as_ref().unwrap().position,
                    book_entry.abookmark.as_ref().unwrap().id,
                ),
            )),
            None => continue,
        }
        println!("{}", book_entry.book.name);
    }
    let select = SelectView::new()
        .with_all(book_select)
        .on_submit(show_player);
    siv.add_layer(Dialog::around(select.scrollable()).title("Select a book to listen"));
}

fn show_check_login(siv: &mut Cursive, email: &str, pass: &str) {
    if email.is_empty() {
        siv.add_layer(Dialog::info("Please enter a email!"));
    } else if pass.is_empty() {
        siv.add_layer(Dialog::info("Please enter a password!"));
    } else {
        client_storytel_api::login(
            siv.user_data::<client_storytel_api::ClientData>().unwrap(),
            email,
            pass,
        );
        if let Err(e) = credentials::save(email, pass) {
            eprintln!("Failed to save credentials: {e}");
        }
        siv.pop_layer();
        siv.add_layer(
            Dialog::around(
                LinearLayout::vertical()
                    .child(Button::new("Bookshelf", show_bookshelf))
                    .child(Button::new("Exit", quit_app)),
            )
            .title("Menu"),
        );
    }
}

pub fn show_login(siv: &mut Cursive) {
    siv.add_layer(
        Dialog::around(
            LinearLayout::vertical()
                .child(TextView::new("Email"))
                .child(EditView::new().with_name("email").fixed_width(20))
                .child(TextView::new("Password"))
                .child(EditView::new().secret().with_name("pass").fixed_width(20)),
        )
        .button("Ok", |s| {
            let email = s
                .call_on_name("email", |view: &mut EditView| view.get_content())
                .unwrap();
            let pass = s
                .call_on_name("pass", |view: &mut EditView| view.get_content())
                .unwrap();
            show_check_login(s, &email, &pass);
        })
        .title("Login"),
    );
}
fn download_current(siv: &mut Cursive) {
    let client_data = siv.user_data::<client_storytel_api::ClientData>().unwrap();

    if let (Some(id), Some(name)) =
        (client_data.current_abook_id, client_data.current_book_name.clone())
    {
        // get final stream url **before** spawning thread
        let stream_url = client_storytel_api::get_stream_url(client_data, &id);

        // progress dialog
        siv.add_layer(
            Dialog::around(
                TextView::new("Starting download …").with_name("download_progress"),
            )
            .title(format!("Downloading \"{name}\"")),
        );

        // prepare sink for UI updates
        let sink = siv.cb_sink().clone();
        std::thread::spawn(move || {
            let start = Instant::now();             // for ETA calculation
            let sink_progress = sink.clone();       // clone for the progress closure

            client_storytel_api::download_stream_with_progress(
                stream_url,
                name.clone(),
                move |downloaded, total| {
                    let elapsed = start.elapsed().as_secs_f64();

                    // compute ETA when possible
                    let eta = if let (Some(t), e) = (total, elapsed) {
                        if downloaded > 0 && e > 0.0 {
                            let speed = downloaded as f64 / e;          // bytes/sec
                            if speed > 0.0 {
                                let remaining = (t - downloaded) as f64;
                                let secs_left = (remaining / speed).round() as u64;
                                Some(format_eta(secs_left))
                            } else { None }
                        } else { None }
                    } else { None };

                    // bytes per second (rounded) → human-readable
                    let speed_bps  = if elapsed > 0.0 {
                        (downloaded as f64 / elapsed).round() as u64
                    } else { 0 };
                    let speed_fmt = format!("{}⁄s", format_bytes(speed_bps));

                    let text = if let Some(t) = total {
                        let pct = (downloaded as f64 / t as f64) * 100.0;
                        let downloaded_fmt = format_bytes(downloaded);
                        let total_fmt = format_bytes(t);
                        match eta {
                            Some(e) => format!("{pct:.1}%  ({downloaded_fmt}/{total_fmt})  {speed_fmt}  ETA {e}"),
                            None     => format!("{pct:.1}%  ({downloaded_fmt}/{total_fmt})  {speed_fmt}"),
                        }
                    } else {
                        let downloaded_fmt = format_bytes(downloaded);
                        match eta {
                            Some(e) => format!("Downloaded {downloaded_fmt}  {speed_fmt}  ETA {e}"),
                            None     => format!("Downloaded {downloaded_fmt}  {speed_fmt}"),
                        }
                    };

                    // send update to UI
                    let _ = sink_progress.send(Box::new(move |s: &mut Cursive| {
                        s.call_on_name("download_progress", |v: &mut TextView| {
                            v.set_content(&text);
                        });
                    }));
                },
            );

            // download finished – inform user
            let _ = sink.send(Box::new(move |s: &mut Cursive| {
                s.pop_layer(); // remove progress dialog
                s.add_layer(Dialog::info(format!("Downloaded \"{name}\"")));
            }));
        });
    } else {
        siv.add_layer(Dialog::info("Nothing to download"));
    }
}
