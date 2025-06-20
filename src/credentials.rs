use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

fn config_path() -> PathBuf {
    if let Ok(xdg_config_home) = env::var("XDG_CONFIG_HOME") {
        Path::new(&xdg_config_home).join("storytel-tui")
    } else if let Some(home_dir) = dirs::home_dir() {
        home_dir.join(".config").join("storytel-tui")
    } else {
        // fallback: current directory
        PathBuf::from("storytel-tui")
    }
}

pub fn save(email: &str, password: &str) -> io::Result<()> {
    let dir = config_path();
    fs::create_dir_all(&dir)?;
    let creds = Credentials {
        email: email.to_string(),
        password: password.to_string(),
    };
    let json = serde_json::to_string(&creds).unwrap();
    let file_path = dir.join("credentials.json");
    let mut file = File::create(file_path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn load() -> Option<(String, String)> {
    let file_path = config_path().join("credentials.json");
    let mut file = File::open(file_path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    let creds: Credentials = serde_json::from_str(&contents).ok()?;
    Some((creds.email, creds.password))
}
