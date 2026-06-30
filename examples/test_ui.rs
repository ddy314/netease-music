use netease_music::{
    CaptchaParams, LoginCellphoneParams, LoginQrCheckParams, NeteaseMusicClient, SearchParams,
    SongQualityLevel, SongUrlV1Params,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

const ADDR: &str = "127.0.0.1:8787";

fn main() -> std::io::Result<()> {
    let client = Arc::new(
        NeteaseMusicClient::builder()
            .build()
            .expect("create NetEase Music client"),
    );
    let listener = TcpListener::bind(ADDR)?;
    println!("Test UI is running at http://{ADDR}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client = Arc::clone(&client);
                std::thread::spawn(move || {
                    if let Err(err) = handle_connection(stream, client) {
                        eprintln!("request failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }

    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    client: Arc<NeteaseMusicClient>,
) -> std::io::Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut content_length = 0usize;

    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            let body_len = buffer.len().saturating_sub(header_end + 4);
            if body_len >= content_length {
                break;
            }
        }
    }

    let Some(header_end) = find_header_end(&buffer) else {
        return write_response(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            "bad request",
        );
    };
    let request_line = String::from_utf8_lossy(&buffer[..header_end])
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    let body = String::from_utf8_lossy(&buffer[body_start..body_end.min(buffer.len())]);
    let form = parse_form(&body);

    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 {
        return write_json(
            &mut stream,
            json!({"ok": false, "error": "invalid request line"}),
        );
    }
    let method = parts[0];
    let path = parts[1].split('?').next().unwrap_or(parts[1]);

    match (method, path) {
        ("GET", "/") => write_response(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            INDEX_HTML,
        ),
        ("GET", "/api/status") => write_json(&mut stream, status_json(&client)),
        ("POST", "/api/login/cellphone") => write_json(&mut stream, login_cellphone(&client, form)),
        ("POST", "/api/captcha/sent") => write_json(&mut stream, captcha_sent(&client, form)),
        ("POST", "/api/qr/key") => write_json(&mut stream, qr_key(&client)),
        ("POST", "/api/qr/check") => write_json(&mut stream, qr_check(&client, form)),
        ("POST", "/api/search") => write_json(&mut stream, search(&client, form)),
        ("POST", "/api/song-url") => write_json(&mut stream, song_url(&client, form)),
        ("POST", "/api/recommend-songs") => write_json(&mut stream, recommend_songs(&client)),
        _ => write_json(&mut stream, json!({"ok": false, "error": "not found"})),
    }
}

fn login_cellphone(client: &NeteaseMusicClient, form: HashMap<String, String>) -> Value {
    let params = LoginCellphoneParams {
        phone: field(&form, "phone"),
        countrycode: optional_field(&form, "countrycode"),
        password: optional_field(&form, "password"),
        md5_password: optional_field(&form, "md5_password"),
        captcha: optional_field(&form, "captcha"),
        csrf_token: None,
    };
    match client.login_cellphone(params) {
        Ok(response) => {
            json!({"ok": true, "status": status_json(client), "response": response.body})
        }
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn captcha_sent(client: &NeteaseMusicClient, form: HashMap<String, String>) -> Value {
    match client.captcha_sent(CaptchaParams {
        phone: field(&form, "phone"),
        countrycode: optional_field(&form, "countrycode"),
        captcha: None,
    }) {
        Ok(response) => json!({"ok": true, "response": response.body}),
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn qr_key(client: &NeteaseMusicClient) -> Value {
    match client.login_qr_key() {
        Ok((response, qrcode_url)) => {
            let unikey = response
                .body
                .get("unikey")
                .and_then(Value::as_str)
                .or_else(|| {
                    response
                        .body
                        .pointer("/data/unikey")
                        .and_then(Value::as_str)
                })
                .unwrap_or_default();
            json!({
                "ok": true,
                "unikey": unikey,
                "qrcodeUrl": qrcode_url,
                "qrImage": qr_image_url(&qrcode_url),
                "response": response.body,
            })
        }
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn qr_check(client: &NeteaseMusicClient, form: HashMap<String, String>) -> Value {
    match client.login_qr_check(LoginQrCheckParams {
        unikey: field(&form, "unikey"),
    }) {
        Ok(response) => {
            json!({"ok": true, "status": status_json(client), "response": response.body})
        }
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn search(client: &NeteaseMusicClient, form: HashMap<String, String>) -> Value {
    match client.search(SearchParams {
        keywords: field(&form, "keywords"),
        search_type: Some(field_or(&form, "search_type", "1")),
        limit: Some(parse_u32(&form, "limit", 10)),
        offset: Some(parse_u32(&form, "offset", 0)),
    }) {
        Ok(response) => json!({"ok": true, "response": response.body}),
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn song_url(client: &NeteaseMusicClient, form: HashMap<String, String>) -> Value {
    let level = match field_or(&form, "level", "higher").as_str() {
        "standard" => SongQualityLevel::Standard,
        "exhigh" => SongQualityLevel::Exhigh,
        "lossless" => SongQualityLevel::Lossless,
        "hires" => SongQualityLevel::Hires,
        "jyeffect" => SongQualityLevel::JyEffect,
        "sky" => SongQualityLevel::Sky,
        "jymaster" => SongQualityLevel::JyMaster,
        _ => SongQualityLevel::Higher,
    };
    match client.song_url_v1(SongUrlV1Params {
        id: field(&form, "id"),
        level: Some(level),
        encode_type: Some(field_or(&form, "encode_type", "flac")),
    }) {
        Ok(response) => {
            let play_url = response
                .body
                .pointer("/data/0/url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            json!({"ok": true, "playUrl": play_url, "response": response.body})
        }
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn recommend_songs(client: &NeteaseMusicClient) -> Value {
    match client.recommend_songs() {
        Ok(response) => json!({"ok": true, "response": response.body}),
        Err(err) => json!({"ok": false, "error": err.to_string()}),
    }
}

fn status_json(client: &NeteaseMusicClient) -> Value {
    json!({
        "loggedIn": client.cookie("MUSIC_U").is_some(),
        "anonymous": client.cookie("MUSIC_A").is_some(),
        "csrf": client.cookie("__csrf").is_some(),
        "cookieNames": client.cookies().into_iter().map(|cookie| cookie.name).collect::<Vec<_>>(),
    })
}

fn parse_form(body: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

fn field(form: &HashMap<String, String>, name: &str) -> String {
    form.get(name).cloned().unwrap_or_default()
}

fn field_or(form: &HashMap<String, String>, name: &str, fallback: &str) -> String {
    form.get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn optional_field(form: &HashMap<String, String>, name: &str) -> Option<String> {
    form.get(name).filter(|value| !value.is_empty()).cloned()
}

fn parse_u32(form: &HashMap<String, String>, name: &str, fallback: u32) -> u32 {
    form.get(name)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(fallback)
}

fn qr_image_url(qrcode_url: &str) -> String {
    let encoded = url::form_urlencoded::byte_serialize(qrcode_url.as_bytes()).collect::<String>();
    format!("https://api.qrserver.com/v1/create-qr-code/?size=220x220&data={encoded}")
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

fn write_json(stream: &mut TcpStream, value: Value) -> std::io::Result<()> {
    write_response(
        stream,
        "200 OK",
        "application/json; charset=utf-8",
        &serde_json::to_string_pretty(&value).expect("serialize JSON"),
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>netease-music 测试台</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f6f7f8;
      --text: #181a1f;
      --muted: #68707d;
      --line: #d9dee5;
      --panel: #ffffff;
      --accent: #d73535;
      --accent-strong: #b82626;
      --ok: #157f4f;
      --shadow: 0 18px 50px rgba(28, 31, 38, 0.08);
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--text);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      line-height: 1.5;
    }
    header {
      padding: 28px clamp(18px, 4vw, 56px) 18px;
      border-bottom: 1px solid var(--line);
      background: rgba(255,255,255,0.78);
      backdrop-filter: blur(18px);
      position: sticky;
      top: 0;
      z-index: 2;
    }
    h1 {
      margin: 0;
      font-size: clamp(24px, 4vw, 42px);
      letter-spacing: 0;
    }
    header p {
      margin: 8px 0 0;
      color: var(--muted);
      max-width: 780px;
    }
    main {
      display: grid;
      grid-template-columns: minmax(280px, 430px) minmax(0, 1fr);
      gap: 22px;
      padding: 24px clamp(18px, 4vw, 56px) 56px;
    }
    section {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      box-shadow: var(--shadow);
      padding: 18px;
    }
    .stack { display: grid; gap: 16px; align-content: start; }
    h2 {
      margin: 0 0 14px;
      font-size: 17px;
      letter-spacing: 0;
    }
    label {
      display: grid;
      gap: 6px;
      margin: 10px 0;
      color: var(--muted);
      font-size: 13px;
    }
    input, select, button {
      width: 100%;
      min-height: 38px;
      border-radius: 6px;
      border: 1px solid var(--line);
      font: inherit;
    }
    input, select {
      padding: 8px 10px;
      background: #fff;
      color: var(--text);
    }
    button {
      cursor: pointer;
      border-color: var(--accent);
      background: var(--accent);
      color: white;
      font-weight: 650;
      transition: background .15s ease, transform .15s ease;
    }
    button:hover { background: var(--accent-strong); transform: translateY(-1px); }
    button.secondary {
      background: #fff;
      color: var(--accent);
    }
    .row {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 10px;
    }
    .status {
      display: flex;
      align-items: center;
      gap: 10px;
      color: var(--muted);
      font-size: 13px;
    }
    .dot {
      width: 9px;
      height: 9px;
      border-radius: 50%;
      background: #9aa3af;
    }
    .dot.ok { background: var(--ok); }
    .qr {
      display: grid;
      grid-template-columns: 120px 1fr;
      gap: 14px;
      align-items: center;
      min-height: 120px;
    }
    .qr img {
      width: 120px;
      height: 120px;
      border: 1px solid var(--line);
      border-radius: 6px;
      background: #fff;
    }
    audio {
      width: 100%;
      margin-top: 12px;
    }
    pre {
      margin: 0;
      min-height: 520px;
      max-height: calc(100vh - 190px);
      overflow: auto;
      padding: 16px;
      border-radius: 8px;
      background: #101319;
      color: #d7e2f2;
      font-size: 12px;
      line-height: 1.55;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .hint {
      color: var(--muted);
      font-size: 12px;
      margin: 8px 0 0;
    }
    @media (max-width: 900px) {
      main { grid-template-columns: 1fr; }
      pre { max-height: none; min-height: 320px; }
    }
  </style>
</head>
<body>
  <header>
    <h1>netease-music 测试台</h1>
    <p>本地快速验证登录状态、搜索结果和歌曲播放地址。所有请求都从当前 Rust client 实例发出。</p>
  </header>
  <main>
    <div class="stack">
      <section>
        <h2>登录状态</h2>
        <div class="status"><span id="statusDot" class="dot"></span><span id="statusText">读取中</span></div>
        <p id="cookieNames" class="hint"></p>
      </section>

      <section>
        <h2>手机号登录</h2>
        <label>手机号 <input id="phone" autocomplete="username" /></label>
        <div class="row">
          <label>国家码 <input id="countrycode" value="86" /></label>
          <label>验证码 <input id="captcha" /></label>
        </div>
        <label>密码 <input id="password" type="password" autocomplete="current-password" /></label>
        <div class="row">
          <button onclick="sendCaptcha()">发送验证码</button>
          <button onclick="loginCellphone()">登录</button>
        </div>
        <p class="hint">如果账号触发风控，可改用二维码登录。</p>
      </section>

      <section>
        <h2>二维码登录</h2>
        <div class="qr">
          <img id="qrImage" alt="二维码" />
          <div>
            <button onclick="createQr()">生成二维码</button>
            <button class="secondary" onclick="checkQr()">检查扫码状态</button>
            <p id="qrText" class="hint"></p>
          </div>
        </div>
      </section>

      <section>
        <h2>搜索</h2>
        <label>关键词 <input id="keywords" value="周杰伦" /></label>
        <div class="row">
          <label>类型
            <select id="searchType">
              <option value="1">单曲</option>
              <option value="10">专辑</option>
              <option value="100">歌手</option>
              <option value="1000">歌单</option>
            </select>
          </label>
          <label>数量 <input id="limit" type="number" value="10" min="1" max="100" /></label>
        </div>
        <button onclick="searchSongs()">搜索</button>
      </section>

      <section>
        <h2>播放测试</h2>
        <label>歌曲 ID <input id="songId" value="33894312" /></label>
        <div class="row">
          <label>音质
            <select id="level">
              <option value="higher">higher</option>
              <option value="exhigh">exhigh</option>
              <option value="lossless">lossless</option>
              <option value="hires">hires</option>
              <option value="standard">standard</option>
            </select>
          </label>
          <label>编码 <input id="encodeType" value="flac" /></label>
        </div>
        <button onclick="loadSongUrl()">获取地址并播放</button>
        <audio id="player" controls></audio>
        <p id="playHint" class="hint"></p>
      </section>
    </div>

    <section>
      <h2>响应</h2>
      <pre id="output">{}</pre>
    </section>
  </main>

  <script>
    let currentUnikey = "";
    const output = document.getElementById("output");

    function form(values) {
      return new URLSearchParams(values);
    }

    async function api(path, values = {}) {
      const res = await fetch(path, {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: form(values)
      });
      const json = await res.json();
      output.textContent = JSON.stringify(json, null, 2);
      if (json.status) renderStatus(json.status);
      return json;
    }

    async function refreshStatus() {
      const res = await fetch("/api/status");
      renderStatus(await res.json());
    }

    function renderStatus(status) {
      document.getElementById("statusDot").className = "dot" + (status.loggedIn ? " ok" : "");
      document.getElementById("statusText").textContent = status.loggedIn ? "已登录" : "未登录";
      document.getElementById("cookieNames").textContent = "cookies: " + status.cookieNames.join(", ");
    }

    async function sendCaptcha() {
      await api("/api/captcha/sent", {
        phone: phone.value,
        countrycode: countrycode.value
      });
    }

    async function loginCellphone() {
      const json = await api("/api/login/cellphone", {
        phone: phone.value,
        countrycode: countrycode.value,
        password: password.value,
        captcha: captcha.value
      });
      if (json.ok) refreshStatus();
    }

    async function createQr() {
      const json = await api("/api/qr/key");
      currentUnikey = json.unikey || "";
      document.getElementById("qrImage").src = json.qrImage || "";
      document.getElementById("qrText").textContent = json.qrcodeUrl || "";
    }

    async function checkQr() {
      const json = await api("/api/qr/check", { unikey: currentUnikey });
      if (json.ok) refreshStatus();
    }

    async function searchSongs() {
      await api("/api/search", {
        keywords: keywords.value,
        search_type: searchType.value,
        limit: limit.value
      });
    }

    async function loadSongUrl() {
      const json = await api("/api/song-url", {
        id: songId.value,
        level: level.value,
        encode_type: encodeType.value
      });
      const player = document.getElementById("player");
      const hint = document.getElementById("playHint");
      if (json.playUrl) {
        player.src = json.playUrl;
        hint.textContent = "已加载播放地址。若浏览器不支持当前编码，换 higher/exhigh 或 encodeType=mp3 再试。";
        player.play().catch(() => {});
      } else {
        player.removeAttribute("src");
        hint.textContent = "未返回可播放 URL，可能需要登录、歌曲无版权或接口返回受限。";
      }
    }

    refreshStatus();
  </script>
</body>
</html>
"#;
