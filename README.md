# netease-music

Rust client library for calling NetEase Cloud Music APIs.

This crate follows the request behavior of `NeteaseCloudMusicApi@4.32.0` while
exposing a stateful Rust `NeteaseMusicClient`, so cookies, device identifiers,
timeout settings, proxy settings, and request encryption are owned by one client
instance.

UNM / music unlocking is intentionally not included. Use an external proxy or a
separate integration if you need that behavior.

## Features

- `weapi`, `eapi`, `linuxapi`, and raw mobile API request helpers.
- Stateful cookie handling, including `Set-Cookie` capture.
- NetEase login context modeled after current NeteaseCloudMusicApi behavior:
  PC login cookies for weapi and interface-host eapi requests with encoded
  client header cookies.
- Wrapped APIs for login, QR login, captcha, search, suggestions, hot search,
  account info, banners, personalized playlists, recommendations, sign-in, song
  URLs, lyrics, scrobble playback reporting, playlist detail, full playlist
  tracks, song detail, user detail, user playlists, like lists, recent records,
  and listen-data reports.
- Raw helpers for endpoints not yet wrapped as first-class Rust methods.

## Install

From crates.io:

```toml
[dependencies]
netease-music = "0.1.1"
```

For local development:

```shell
cargo add --path .
```

## Usage

```rust
use netease_music::{NeteaseMusicClient, Result, SearchParams, SongUrlV1Params};

fn main() -> Result<()> {
    let client = NeteaseMusicClient::new()?;

    let search = client.search(SearchParams {
        keywords: "周杰伦".to_string(),
        ..Default::default()
    })?;
    println!("{}", search.body);

    let url = client.song_url_v1(SongUrlV1Params {
        id: "33894312".to_string(),
        ..Default::default()
    })?;
    println!("{}", url.body);

    Ok(())
}
```

## QR Login

```rust
use netease_music::{LoginQrCheckParams, NeteaseMusicClient, Result};

fn main() -> Result<()> {
    let client = NeteaseMusicClient::new()?;
    let (_key_response, qrcode_url) = client.login_qr_key()?;
    println!("scan this URL as a QR code: {qrcode_url}");

    // Poll after the mobile app scans and confirms.
    let response = client.login_qr_check(LoginQrCheckParams {
        unikey: "unikey from login_qr_key response".to_string(),
    })?;
    println!("{}", response.body);
    Ok(())
}
```

Use the same `NeteaseMusicClient` instance for QR key generation and polling so
the login cookies and device identifiers stay consistent.

`login_qr_key()` returns the PC QR URL. Use `login_qr_url(key, Some("web"))` if
you specifically need the web `chainId` URL.

## Raw APIs

Use raw helpers while a specific endpoint wrapper is not yet available:

```rust
use netease_music::{NeteaseMusicClient, Result};
use serde_json::json;

fn main() -> Result<()> {
    let client = NeteaseMusicClient::new()?;
    let response = client.raw_weapi(
        "https://music.163.com/weapi/search/hot",
        json!({}),
    )?;
    println!("{}", response.body);
    Ok(())
}
```

Available raw helpers:

- `raw_weapi(url, data)`
- `raw_eapi(url, data)`
- `eapi_with_path(url, crypto_path, data)`
- `raw_linuxapi(method, url, data)`
- `raw_api(url, data)`

## Wrapped APIs

- Login: `login_cellphone`, `login_qr_key`, `login_qr_url`,
  `login_qr_check`, `login_status`, `login_refresh`, `logout`,
  `register_anonymous`
- Captcha: `captcha_sent`, `captcha_verify`
- Discovery/search: `search`, `search_suggest`, `search_hot_detail`,
  `banner`, `personalized`, `recommend_songs`, `recommend_resource`
- Library/playback: `song_url`, `song_url_v1`, `lyric`, `scrobble`,
  `record_recent_song`, `recent_listen_list`, `playlist_detail`,
  `playlist_track_all`, `song_detail`
- User/account: `account`, `user_detail`, `user_playlist`, `like_list`,
  `daily_signin`
- Listen data: `listen_data_report`, `listen_data_realtime_report`,
  `listen_data_total`, `listen_data_today_song_play_rank`,
  `listen_data_year_report`

## Development

```shell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo package --allow-dirty
```

`cargo package --allow-dirty` verifies the package contents locally. Use
`cargo publish --dry-run` before publishing to crates.io.

## Test UI

Run the local test interface:

```shell
cargo run --example test_ui
```

Then open:

```text
http://127.0.0.1:8787
```

The page can test cellphone login, QR login, captcha sending, search, song URL
lookup, and browser playback through an `<audio>` element. If a returned URL
does not play, try `higher` or `exhigh` quality, or set `encodeType` to `mp3`.
