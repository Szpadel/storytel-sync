use crate::password_crypt;

type SenderType = ();

type ReceiverType = ();
use serde::Deserialize;

pub struct ClientData {
    pub request_client: reqwest::Client,
    pub login_data: Login,
    #[allow(dead_code)]
    pub sender: Option<SenderType>,
    #[allow(dead_code)]
    pub receiver: Option<ReceiverType>,
    #[allow(dead_code)]
    pub current_abookmark_id: Option<u64>,
    #[allow(dead_code)]
    pub current_abook_id: Option<u64>,
    #[allow(dead_code)]
    pub current_book_name: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountInfo {
    #[serde(rename = "singleSignToken")]
    pub single_sign_token: String,
}

#[derive(Deserialize)]
pub struct Login {
    #[serde(rename = "accountInfo")]
    pub account_info: AccountInfo,
}

#[derive(Deserialize)]
pub struct BookShelf {
    #[serde(rename = "books")]
    pub books: Vec<BookEntry>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct BookEntry {
    pub abook: Option<Abook>,
    #[serde(rename = "abookMark")]
    pub abookmark: Option<AbookMark>,
    pub book: Book,
    #[serde(rename = "isbn")]
    pub isbn: Option<String>,
    #[serde(rename = "cover")]
    pub cover: Option<String>,
    #[serde(rename = "length")]
    pub length: Option<u64>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "author")]
    pub author: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct AbookMark {
    #[serde(rename = "bookId")]
    pub id: u64,
    #[serde(rename = "pos")]
    pub position: i64,
}

#[derive(Deserialize)]
pub struct Abook {
    pub id: u64,
    #[serde(rename = "isbn")]
    pub isbn: Option<String>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct Book {
    pub name: String,
    #[serde(rename = "isbn")]
    pub isbn: Option<String>,
    #[serde(rename = "authorsAsString")]
    pub authors_as_string: Option<String>,
    #[serde(rename = "cover")]
    pub cover: Option<String>,
    #[serde(rename = "length")]
    pub length: Option<u64>,
    #[serde(rename = "description")]
    pub description: Option<String>,
}

pub async fn login(client_data: &mut ClientData, email: &str, pass: &str) -> eyre::Result<()> {
    let hex_encryp_pass = password_crypt::encrypt_password(pass.trim());

    let url = format!(
        "https://www.storytel.com/api/login.action\
                      ?m=1&uid={}&pwd={}",
        email.trim(),
        hex_encryp_pass
    );

    let resp_login = client_data.request_client.get(&url).send().await?;
    client_data.login_data = resp_login.json::<Login>().await?;
    Ok(())
}

pub async fn get_bookshelf(client_data: &mut ClientData) -> eyre::Result<BookShelf> {
    let url_get_bookshelf = format!(
        "https://www.storytel.com/api/getBookShelf.\
                                    action?token={}",
        client_data.login_data.account_info.single_sign_token
    );
    let resp_bookshelf = client_data.request_client.get(&url_get_bookshelf).send().await?;
    Ok(resp_bookshelf.json::<BookShelf>().await?)
}

pub async fn get_stream_url(client_data: &mut ClientData, id: u64) -> eyre::Result<String> {
    let url_ask_stream = format!(
        "https://www.storytel.com/mp3streamRangeReq\
                                 ?startposition=0&programId={}&token={}",
        id, client_data.login_data.account_info.single_sign_token
    );

    let resp = client_data.request_client.get(&url_ask_stream).send().await?;
    let loc = resp
        .headers()
        .get("location")
        .ok_or_else(|| eyre::eyre!("missing location header"))?
        .to_str()?
        .to_string();
    Ok(loc)
}

use std::path::Path;

pub async fn download_stream_with_progress<F>(
    stream_url: &str,
    book_path: &Path,
    mut progress: F,
) -> eyre::Result<()>
where
    F: FnMut(u64, Option<u64>) + Send + 'static,
{
    use futures_util::StreamExt;
    use tokio::{fs, io::AsyncWriteExt};

    tracing::debug!(
        "download_stream_with_progress: url={}, dst={:?}",
        stream_url, book_path
    );

    fs::create_dir_all(book_path).await?;
    let mut file = fs::File::create(book_path.join("audio.mp3")).await?;

    let resp = reqwest::get(stream_url).await?;
    let total = resp.content_length();
    let mut downloaded = 0u64;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        progress(downloaded, total);
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn set_bookmark(client_data: &mut ClientData, position: i64) -> eyre::Result<()> {
    let microsec_to_sec = 1_000_000;
    let params = [
        (
            "token",
            client_data
                .login_data
                .account_info
                .single_sign_token
                .to_string(),
        ),
        (
            "bookId",
            client_data.current_abookmark_id.unwrap().to_string(),
        ),
        ("pos", (position * microsec_to_sec).to_string()),
        ("type", "1".to_string()),
    ];
    let url_set_bookmark = "https://www.storytel.se/api/setABookmark.action".to_string();
    client_data
        .request_client
        .post(url_set_bookmark)
        .form(&params)
        .send()
        .await?;
    Ok(())
}
