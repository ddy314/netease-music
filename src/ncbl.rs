use flate2::{write::GzEncoder, Compression};
use num_bigint::BigUint;
use rand::{rngs::OsRng, Rng, RngCore};
use serde_json::{json, Value};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::services::ScrobbleParams;
use crate::{ApiResponse, Cookie, NeteaseError, NeteaseMusicClient, Result};

const RSA_N_HEX: &str = "fd90bd466ff9bc8a3fec2fbcf263b90d5c564879fa5d7aab89b31c1d5cb4139d";
const FIELD_SEP: char = '\x01';

#[derive(Clone)]
struct Ctx {
    app_version: String,
    app_version_code: String,
    app_channel: String,
    app_nsm: String,
    app_cid: String,
    device_id: String,
    device_ti: String,
    device_sign: String,
    device_model: String,
    device_nnid: String,
    device_nuid: String,
    device_csrf: String,
    device_os: String,
    device_osver: String,
    auth_token: String,
    auth_session: String,
    auth_vip_type: String,
}

pub(crate) fn report_scrobble_v1(
    client: &NeteaseMusicClient,
    params: &ScrobbleParams,
) -> Result<ApiResponse> {
    let song_id = params.id.parse::<u64>().map_err(|_| {
        NeteaseError::InvalidOption("scrobble id must be a numeric song id".to_string())
    })?;
    let ctx = Ctx::from_cookies(client.cookies())?;
    let source_id = if params.sourceid.is_empty() {
        params.id.as_str()
    } else {
        params.sourceid.as_str()
    };
    let song = json!({
        "id": song_id,
        "bitrate": 320,
        "level": "exhigh",
        "time": params.time,
    });
    let source = json!({
        "id": source_id,
        "type": "track",
        "name": "list",
    });
    let meta = build_meta_json(&ctx);
    let cookie = build_cookie_string(&ctx);
    let ts = now_secs();

    let plv_body = build_records(ts, "_plv", build_plv(&ctx, &song, &source));
    let plv = upload(client, &ctx, &meta, &plv_body, &cookie)?;
    if !upload_success(&plv) {
        return Ok(plv);
    }

    let pld_body = build_records(
        ts,
        "_pld",
        build_pld(
            &ctx,
            &song,
            &source,
            params.time.min(song["time"].as_u64().unwrap_or(0) as u32),
        ),
    );
    let pld = upload(client, &ctx, &meta, &pld_body, &cookie)?;
    if !upload_success(&pld) {
        return Ok(api_response(
            200,
            json!({"code": pld.code.unwrap_or(-1), "msg": "PLV succeeded but PLD failed", "details": {"plv": plv.body, "pld": pld.body}}),
        ));
    }

    Ok(api_response(
        200,
        json!({"code": 200, "data": "scrobble_v1 success", "details": {"plv": plv.body, "pld": pld.body}}),
    ))
}

impl Ctx {
    fn from_cookies(cookies: Vec<Cookie>) -> Result<Self> {
        let get = |name: &str| {
            cookies
                .iter()
                .find(|cookie| cookie.name == name)
                .map(|cookie| cookie.value.clone())
                .unwrap_or_default()
        };
        let token = get("MUSIC_U");
        if token.is_empty() {
            return Err(NeteaseError::InvalidOption(
                "MUSIC_U cookie is required for NCBL scrobble".to_string(),
            ));
        }
        let nuid = get("_ntes_nuid");
        Ok(Self {
            app_version: empty_or(get("appver"), "3.1.35"),
            app_version_code: empty_or(get("versioncode"), "205293"),
            app_channel: empty_or(get("channel"), "netease"),
            app_nsm: empty_or(get("WEVNSM"), "1.0.0"),
            app_cid: empty_or(
                get("WNMCID"),
                &format!("{}.{}.01.0", random_hex(3), now_millis()),
            ),
            device_id: get("deviceId").or_else_empty(get("sDeviceId")),
            device_ti: get("NMTID"),
            device_sign: get("clientSign"),
            device_model: get("mode").or_else_empty(get("mobilename")),
            device_nnid: empty_or(get("_ntes_nnid"), ","),
            device_nuid: nuid,
            device_csrf: get("__csrf"),
            device_os: "pc".to_string(),
            device_osver: empty_or(
                get("osver"),
                "Microsoft-Windows-10-Professional-build-19045-64bit",
            ),
            auth_token: token,
            auth_session: get("JSESSIONID-WYYY"),
            auth_vip_type: get("vipType"),
        })
    }
}

trait OrElseEmpty {
    fn or_else_empty(self, fallback: String) -> String;
}

impl OrElseEmpty for String {
    fn or_else_empty(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn build_plv(ctx: &Ctx, song: &Value, source: &Value) -> Value {
    let now = now_millis();
    json!({
        "mode": "circulation",
        "download": 0,
        "alg": "",
        "status": "front",
        "id": song["id"].to_string(),
        "bitrate": song["bitrate"],
        "type": "song",
        "is_listentogether": 0,
        "source": source["name"],
        "is_heart": 0,
        "resource_ratio": "",
        "resource_time": song["time"],
        "musiceffect_id": "",
        "app_mode": 2,
        "bitrate_level": song["level"],
        "_addrefer": format!("[F:63][{now}#933#{}#{}#c9156c3][e][2][23][cell_pc_songlist_song:2|page_pc_songlist_songflow|page_mine_like_music][{}:song:x:x|:::|{}:list::]", ctx.app_version, ctx.app_version_code, song["id"], source["id"]),
        "_multirefers": ["[F:26][s][18][_ai]", "[F:26][s][12][_ai]", "[F:26][s][5][_ai]", "[F:26][s][0][_ai]"],
        "vipType": ctx.auth_vip_type,
        "fee": 1,
        "file": 4,
        "rightSource": 0,
        "sourceId": source["id"],
        "sourcetype": source["type"],
        "libra_abt": "",
        "channel": ctx.app_channel,
        "curStartChannel": "",
    })
}

fn build_pld(ctx: &Ctx, song: &Value, source: &Value, played: u32) -> Value {
    let now = now_millis();
    json!({
        "mode": "circulation",
        "download": 0,
        "alg": "",
        "status": "front",
        "id": song["id"].to_string(),
        "time": played,
        "type": "song",
        "is_listentogether": 0,
        "source": source["name"],
        "is_heart": 0,
        "realtime": played,
        "resource_ratio": "",
        "resource_time": song["time"],
        "musiceffect_id": "1001",
        "app_mode": 1,
        "lyriceffect": "default",
        "displayMode": "classic",
        "bitrate": song["bitrate"],
        "bitrate_level": song["level"],
        "_addrefer": format!("[F:63][{now}#616#{}#{}#c9156c3][e][2][92][btn_pc_cover_play|cell_pc_songlist_song:6|page_pc_songlist_songflow|page_mine_like_music][:::|{}:song:x:x|:::|{}:list::]", ctx.app_version, ctx.app_version_code, song["id"], source["id"]),
        "_multirefers": ["[F:26][s][87][_ai]", "[F:26][s][81][_ai]", "[F:26][s][75][_ai]", "[F:26][s][69][_ai]", "[F:26][s][63][_ai]"],
        "vipType": ctx.auth_vip_type,
        "fee": 8,
        "file": 4,
        "rightSource": 0,
        "sourceId": source["id"],
        "sourcetype": source["type"],
        "end": "interrupt",
        "libra_abt": "",
        "channel": ctx.app_channel,
        "curStartChannel": "",
    })
}

fn build_records(time: u64, action: &str, data: Value) -> String {
    format!("{time}{FIELD_SEP}{action}{FIELD_SEP}{data}")
}

fn upload(
    client: &NeteaseMusicClient,
    ctx: &Ctx,
    meta: &str,
    body: &str,
    cookie: &str,
) -> Result<ApiResponse> {
    let payload = encrypt_ncbl(meta.as_bytes(), body.as_bytes())?;
    let boundary = random_hex(16);
    let file_name = format!(
        "op_{}_0_{}",
        rand::thread_rng().gen_range(10000..99999),
        rand::thread_rng().gen_range(1..u32::MAX)
    );
    let multipart = multipart(&boundary, &file_name, &payload);
    let response = client.post_bytes(
        "https://clientlog3.music.163.com/api/clientlog/encrypt/upload?multiupload=true",
        vec![
            (
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ),
            ("referer", "https://music.163.com/di".to_string()),
            (
                "user-agent",
                format!("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Safari/537.36 Chrome/91.0.4472.164 NeteaseMusicDesktop/{}", ctx.app_version),
            ),
            ("accept-language", "zh-CN,zh;q=0.8".to_string()),
            ("cookie", cookie.to_string()),
        ],
        multipart,
    )?;
    Ok(api_response(
        response.status,
        json!({
            "code": response.code.unwrap_or(-1),
            "fileName": file_name,
            "payloadSize": payload.len(),
            "response": response.body,
        }),
    ))
}

fn upload_success(response: &ApiResponse) -> bool {
    let code = response.body["code"].as_i64() == Some(200);
    let file = response.body["fileName"].as_str().unwrap_or_default();
    code && response.body["response"]["data"]["successfiles"]
        .as_array()
        .map(|files| files.iter().any(|value| value.as_str() == Some(file)))
        .unwrap_or(false)
}

fn encrypt_ncbl(meta: &[u8], body: &[u8]) -> Result<Vec<u8>> {
    let mut key_a = random_bytes(32);
    if key_a[0] >= 0xa3 {
        key_a[0] = 0xa2;
    }
    let key_b = rsa_wrap(&key_a)?;
    let mut uuid = random_bytes(16);
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    let nonce = &uuid[..12];
    let counter = u32::from_le_bytes(uuid[12..16].try_into().unwrap()) >> 2;
    let base_seq = rand::thread_rng().gen::<u16>() as u32;

    let meta_cipher = chacha20(&key_b, counter, nonce, meta);
    let mut meta_block = Vec::with_capacity(4 + meta_cipher.len());
    meta_block.extend_from_slice(&0x4343u16.to_le_bytes());
    meta_block.extend_from_slice(&(meta_cipher.len() as u16).to_le_bytes());
    meta_block.extend_from_slice(&meta_cipher);

    let compressed = gzip(body)?;
    let mut trailing = Vec::new();
    let mut seq = base_seq;
    for chunk in compressed.chunks(0x8000) {
        let cipher = chacha20(&key_a, counter, nonce, chunk);
        trailing.extend_from_slice(&(cipher.len() as u16).to_le_bytes());
        trailing.extend_from_slice(&seq.to_le_bytes());
        trailing.extend_from_slice(&cipher);
        seq += 1;
    }

    let header_len = 70 + meta_block.len();
    let mut header = vec![0u8; 70];
    header[..4].copy_from_slice(b"NCBL");
    header[4..8].copy_from_slice(&3u32.to_le_bytes());
    header[8..10].copy_from_slice(&(header_len as u16).to_le_bytes());
    header[10..26].copy_from_slice(&uuid);
    header[26..58].copy_from_slice(&key_b);
    header[58..62].copy_from_slice(&base_seq.to_le_bytes());
    header[62..66].copy_from_slice(&(seq - 1).to_le_bytes());
    header[66..70].copy_from_slice(&(trailing.len() as u32).to_le_bytes());

    let mut out = header;
    out.extend_from_slice(&meta_block);
    out.extend_from_slice(&trailing);
    Ok(out)
}

fn chacha20(key: &[u8], counter: u32, nonce: &[u8], data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    for (block_idx, chunk) in data.chunks(64).enumerate() {
        let key_stream = chacha_block(key, counter.wrapping_add(block_idx as u32), nonce);
        for (idx, byte) in chunk.iter().enumerate() {
            out[block_idx * 64 + idx] = byte ^ key_stream[idx];
        }
    }
    out
}

fn chacha_block(key: &[u8], counter: u32, nonce: &[u8]) -> [u8; 64] {
    let mut state = [0u32; 16];
    state[0..4].copy_from_slice(&[0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
    for idx in 0..8 {
        state[4 + idx] = u32::from_le_bytes(key[idx * 4..idx * 4 + 4].try_into().unwrap());
    }
    state[12] = counter;
    state[13] = u32::from_le_bytes(nonce[0..4].try_into().unwrap());
    state[14] = u32::from_le_bytes(nonce[4..8].try_into().unwrap());
    state[15] = u32::from_le_bytes(nonce[8..12].try_into().unwrap());

    let mut work = state;
    for _ in 0..10 {
        quarter_round(&mut work, 0, 4, 8, 12);
        quarter_round(&mut work, 1, 5, 9, 13);
        quarter_round(&mut work, 2, 6, 10, 14);
        quarter_round(&mut work, 3, 7, 11, 15);
        quarter_round(&mut work, 0, 5, 10, 15);
        quarter_round(&mut work, 1, 6, 11, 12);
        quarter_round(&mut work, 2, 7, 8, 13);
        quarter_round(&mut work, 3, 4, 9, 14);
    }
    let mut out = [0u8; 64];
    for idx in 0..16 {
        out[idx * 4..idx * 4 + 4]
            .copy_from_slice(&work[idx].wrapping_add(state[idx]).to_le_bytes());
    }
    out
}

fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

fn rsa_wrap(key: &[u8]) -> Result<Vec<u8>> {
    let modulus = BigUint::parse_bytes(RSA_N_HEX.as_bytes(), 16)
        .ok_or_else(|| NeteaseError::Crypto("invalid NCBL RSA modulus".to_string()))?;
    let encrypted = BigUint::from_bytes_be(key).modpow(&BigUint::from(65_537u32), &modulus);
    let mut out = encrypted.to_bytes_be();
    if out.len() < 32 {
        let mut padded = vec![0u8; 32 - out.len()];
        padded.extend_from_slice(&out);
        out = padded;
    }
    Ok(out)
}

fn gzip(body: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body)?;
    Ok(encoder.finish()?)
}

fn multipart(boundary: &str, file_name: &str, payload: &[u8]) -> Vec<u8> {
    let head = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: multipart/form-data\r\n\r\n\r\n"
    );
    let tail = format!("\r\n--{boundary}--\r\n");
    [head.as_bytes(), payload, tail.as_bytes()].concat()
}

fn build_cookie_string(ctx: &Ctx) -> String {
    [
        ("JSESSIONID-WYYY", ctx.auth_session.as_str()),
        ("MUSIC_U", ctx.auth_token.as_str()),
        ("NMTID", ctx.device_ti.as_str()),
        ("WEVNSM", ctx.app_nsm.as_str()),
        ("WNMCID", ctx.app_cid.as_str()),
        ("__csrf", ctx.device_csrf.as_str()),
        ("__remember_me", "true"),
        ("_iuqxldmzr_", "33"),
        ("_ntes_nnid", ctx.device_nnid.as_str()),
        ("_ntes_nuid", ctx.device_nuid.as_str()),
        (
            "appver",
            &format!("{}.{}", ctx.app_version, ctx.app_version_code),
        ),
        ("channel", ctx.app_channel.as_str()),
        ("clientSign", ctx.device_sign.as_str()),
        ("deviceId", ctx.device_id.as_str()),
        ("mode", ctx.device_model.as_str()),
        ("ntes_kaola_ad", "1"),
        ("os", ctx.device_os.as_str()),
        ("osver", ctx.device_osver.as_str()),
    ]
    .into_iter()
    .map(|(name, value)| format!("{name}={value}"))
    .collect::<Vec<_>>()
    .join("; ")
}

fn build_meta_json(ctx: &Ctx) -> String {
    json!({
        "JSESSIONID-WYYY": ctx.auth_session,
        "MUSIC_U": ctx.auth_token,
        "NMTID": ctx.device_ti,
        "WEVNSM": ctx.app_nsm,
        "WNMCID": ctx.app_cid,
        "__csrf": ctx.device_csrf,
        "_iuqxldmzr_": "33",
        "_ntes_nnid": ctx.device_nnid,
        "_ntes_nuid": ctx.device_nuid,
        "appver": format!("{}.{}", ctx.app_version, ctx.app_version_code),
        "channel": ctx.app_channel,
        "clientSign": ctx.device_sign,
        "deviceId": ctx.device_id,
        "mode": ctx.device_model,
        "ntes_kaola_ad": "1",
        "os": ctx.device_os,
        "osver": ctx.device_osver,
    })
    .to_string()
}

fn api_response(status: u16, body: Value) -> ApiResponse {
    let raw = serde_json::to_vec(&body).unwrap_or_default();
    let code = body.get("code").and_then(Value::as_i64);
    ApiResponse {
        status,
        code,
        body,
        raw: raw.into(),
        cookies: Vec::new(),
    }
}

fn empty_or(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    OsRng.fill_bytes(&mut out);
    out
}

fn random_hex(len: usize) -> String {
    hex::encode(random_bytes(len))
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn now_secs() -> u64 {
    (now_millis() / 1000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncbl_payload_has_magic_header() {
        let payload = encrypt_ncbl(b"{}", b"1\x01_plv\x01{}").unwrap();
        assert_eq!(&payload[..4], b"NCBL");
        assert_eq!(u32::from_le_bytes(payload[4..8].try_into().unwrap()), 3);
    }

    #[test]
    fn record_shape_matches_clientlog_format() {
        assert_eq!(
            build_records(7, "_pld", json!({"id":1})),
            "7\x01_pld\x01{\"id\":1}"
        );
    }
}
