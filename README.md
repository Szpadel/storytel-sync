# storytel-sync  [![license]][GPL-3.0]

**Sync and browse your Storytel library from a tiny self-hosted web app.**

## Overview

storytel-sync periodically mirrors your Storytel bookshelf to your own disk and exposes a
minimal web UI so any third-party audiobook player (Smart TVs, car head-units, mobile apps,
etc.) can stream or download the files even on platforms not officially supported by Storytel.

The project started as *storytel-tui* by Javier Sánchez Parra - many thanks for the original
implementation!  It has since been rewritten to an asynchronous Rust web service, dropped the
MPV and OpenSSL dependencies, and gained automatic background synchronisation.

## Supported platforms

The app is developed and tested on GNU/Linux x86-64.  It should run anywhere Rust and
`actix-web` work (macOS, ARM SBCs, …) but is presently untested.

## Features

• Responsive bookshelf web page
• One-click on-demand download
• 24 h periodic background sync
• Single static binary - no external media players required

## Configuration

Create a `config.toml` (or `.json`) file:

```toml
email        = "me@example.com"
password     = "my-storytel-password"
download_dir = "/srv/audiobooks"
sync_enabled = true          # optional, default = false
```

Pass the file on start-up:
`storytel-sync --config /path/to/config.toml`

## Running

### Native

```bash
cargo build --release
target/release/storytel-sync --config ./config.toml
```

Only the standard Rust tool-chain is required.

### Docker

```
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  -v $(pwd)/downloads:/downloads \
  ghcr.io/<org>/storytel-sync:latest \
  --config /app/config.toml
```


## License

The source code of this project is licensed under the GNU General Public License v3.0.
