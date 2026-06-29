# netease-music

Rust client library for calling NetEase Cloud Music APIs.

This crate is a Rust rewrite of the reusable `service` and `util` package from
`github.com/go-musicfox/netease-music`, itself derived from
[sirodeneko/NeteaseCloudMusicApiWithGo](https://github.com/sirodeneko/NeteaseCloudMusicApiWithGo).
The old Go package used a global cookie jar and many service structs. This
crate exposes a stateful `NeteaseMusicClient` instead, so cookies, device
identifiers, timeout settings, proxy settings, and request encryption are owned
by one client instance.

UNM / music unlocking is intentionally not included. Use an external proxy or a
separate integration if you need that behavior.

## Features

- `weapi`, `eapi`, `linuxapi`, and raw mobile API request helpers.
- Stateful cookie handling, including `Set-Cookie` capture.
- NetEase login context modeled after the Go project: `sDeviceId`, QR
  `chainId`, PC login cookies, randomized `NMTID`, and browser-like
  `_ntes_nuid`.
- Wrapped APIs for login, QR login, captcha, search, suggestions, hot search,
  account info, banners, personalized playlists, recommendations, sign-in, song
  URLs, lyrics, playlist detail, full playlist tracks, song detail, user detail,
  user playlists, and like lists.
- Raw helpers for endpoints not yet wrapped as first-class Rust methods.

## Install

From git:

```toml
[dependencies]
netease-music = { git = "https://github.com/ddy314/netease-music.git" }
```

For local development:

```shell
cargo add --path .
```

After publishing to crates.io:

```toml
[dependencies]
netease-music = "0.1"
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

- Login: `login_cellphone`, `login_qr_key`, `login_qr_check`,
  `login_refresh`, `logout`
- Captcha: `captcha_sent`, `captcha_verify`
- Discovery/search: `search`, `search_suggest`, `search_hot_detail`,
  `banner`, `personalized`, `recommend_songs`, `recommend_resource`
- Library/playback: `song_url`, `song_url_v1`, `lyric`, `playlist_detail`,
  `playlist_track_all`, `song_detail`
- User/account: `account`, `user_detail`, `user_playlist`, `like_list`,
  `daily_signin`

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
