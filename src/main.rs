#![feature(let_chains)]

mod client_storytel_api;
mod config;
mod download;
mod password_crypt;
mod web_app;

use std::path::Path;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();

    let client = reqwest::Client::builder()
        .user_agent("okhttp/3.12.8")
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let login_data = client_storytel_api::Login {
        account_info: client_storytel_api::AccountInfo {
            single_sign_token: String::new(),
        },
    };

    let args = clap::Command::new("storytel")
        .arg(
            clap::Arg::new("config")
                .long("config")
                .required(true)
                .value_name("FILE")
                .num_args(1),
        )
        .get_matches();

    let cfg_path = args.get_one::<String>("config").unwrap();
    let app_cfg = config::Config::load(Path::new(cfg_path))?;

    let (sender, receiver) = (None, None);

    let mut client_data = client_storytel_api::ClientData {
        request_client: client,
        login_data,
        sender,
        receiver,
        current_abookmark_id: None,
        current_abook_id: None,
        current_book_name: None,
    };

    // authenticate once so subsequent API calls have a token
    client_storytel_api::login(&mut client_data, &app_cfg.email, &app_cfg.password).await?;
    web_app::run(client_data, &app_cfg).await;
    Ok(())
}
