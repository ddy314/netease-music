use bytes::Bytes;
use flate2::read::ZlibDecoder;
use rand::Rng;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{
    HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, REFERER, SET_COOKIE, USER_AGENT,
};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

use crate::crypto::{eapi_params, linuxapi_params, to_json_value, weapi_params};
use crate::{ApiResponse, Cookie, CryptoMode, RequestOptions, Result};

const IOS_APP_VERSION: &str = "9.0.65";
const MUSIC_HOST: &str = "https://music.163.com";

#[derive(Debug, Clone)]
pub struct ClientBuilder {
    timeout: Option<Duration>,
    proxy: Option<String>,
    cookies: Vec<Cookie>,
}

impl ClientBuilder {
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    pub fn cookie(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.cookies.push(Cookie::new(name, value));
        self
    }

    pub fn build(self) -> Result<NeteaseMusicClient> {
        let mut builder = Client::builder();
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(proxy) = self.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }
        let client = builder.build()?;
        let out = NeteaseMusicClient {
            http: client,
            cookies: Arc::new(Mutex::new(HashMap::new())),
            device_id: random_device_id(),
        };
        out.ensure_cookie("sDeviceId", generate_s_device_id());
        for cookie in self.cookies {
            out.set_cookie(cookie.name, cookie.value);
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct NeteaseMusicClient {
    http: Client,
    cookies: Arc<Mutex<HashMap<String, String>>>,
    device_id: String,
}

impl Default for NeteaseMusicClient {
    fn default() -> Self {
        Self::new().expect("default client should be constructible")
    }
}

impl NeteaseMusicClient {
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder {
            timeout: None,
            proxy: None,
            cookies: Vec::new(),
        }
    }

    pub fn set_cookie(&self, name: impl Into<String>, value: impl Into<String>) {
        self.cookies
            .lock()
            .expect("cookie lock")
            .insert(name.into(), value.into());
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        self.cookies.lock().expect("cookie lock").get(name).cloned()
    }

    pub fn cookies(&self) -> Vec<Cookie> {
        self.cookies
            .lock()
            .expect("cookie lock")
            .iter()
            .map(|(name, value)| Cookie::new(name.clone(), value.clone()))
            .collect()
    }

    pub fn apply_request_strategy(&self) {
        self.set_cookie("os", "pc");
        self.set_cookie("appver", "");
        self.ensure_cookie("NMTID", random_alnum_hex(16));
        self.ensure_cookie("_ntes_nuid", create_ntes_nuid());
    }

    pub fn prepare_login_context(&self) {
        self.apply_request_strategy();
    }

    pub fn csrf_token(&self) -> String {
        self.cookie("__csrf").unwrap_or_default()
    }

    pub fn chain_id(&self) -> String {
        format!(
            "v1_{}_web_login_{}",
            self.cookie("sDeviceId")
                .unwrap_or_else(generate_s_device_id),
            now_millis()
        )
    }

    pub fn call_weapi(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.request("POST", url, data, RequestOptions::new(CryptoMode::Weapi))
    }

    pub fn call_eapi(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.request(
            "POST",
            url,
            data,
            RequestOptions::new(CryptoMode::Eapi).crypto_url(eapi_path(url)),
        )
    }

    pub fn call_linuxapi(&self, method: &str, url: &str, data: Value) -> Result<ApiResponse> {
        self.request(method, url, data, RequestOptions::new(CryptoMode::Linuxapi))
    }

    pub fn call_api(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.request("POST", url, data, RequestOptions::new(CryptoMode::None))
    }

    pub fn request(
        &self,
        method: &str,
        url: &str,
        mut data: Value,
        options: RequestOptions,
    ) -> Result<ApiResponse> {
        let mut target_url = url.to_string();
        let method_upper = method.to_ascii_uppercase();
        let mut headers = self.base_headers(options.user_agent);
        let mut form = BTreeMap::<String, String>::new();

        match options.crypto {
            CryptoMode::Weapi => {
                add_csrf(&mut data, &self.csrf_token());
                form = weapi_params(&data)?;
                target_url = replace_api_segment(&target_url, "/weapi/");
            }
            CryptoMode::Linuxapi => {
                let api_url = replace_api_segment(&target_url, "/api/");
                form = linuxapi_params(&method_upper, &api_url, &data)?;
                target_url = "https://music.163.com/api/linux/forward".to_string();
                headers.insert(
                    USER_AGENT,
                    HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36"),
                );
            }
            CryptoMode::Eapi => {
                let crypto_url = options.crypto_url.unwrap_or_else(|| eapi_path(&target_url));
                let header = self.eapi_cookie_header();
                if let Value::Object(ref mut map) = data {
                    map.insert("header".to_string(), to_json_value(header.clone()));
                }
                for (name, value) in header {
                    self.set_cookie(name, value.as_str().unwrap_or_default().to_string());
                }
                form = eapi_params(&crypto_url, &data)?;
                target_url = replace_api_segment(&target_url, "/eapi/");
            }
            CryptoMode::None => {
                if let Value::Object(map) = data {
                    for (key, value) in map {
                        form.insert(key, stringify_json_value(value));
                    }
                }
            }
        }

        if method_upper == "POST" {
            headers.insert(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            );
        }

        let parsed = Url::parse(&target_url)?;
        let mut req = if method_upper == "GET" {
            self.http.get(parsed)
        } else {
            self.http.post(parsed).form(&form)
        };
        req = self.attach_headers_and_cookies(req, headers)?;

        let response = req.send()?;
        let status = response.status().as_u16();
        let set_cookies = collect_set_cookies(response.headers());
        for cookie in &set_cookies {
            self.set_cookie(cookie.name.clone(), cookie.value.clone());
        }

        let mut raw = response.bytes()?;
        if let Ok(decoded) = maybe_zlib_decode(&raw) {
            raw = Bytes::from(decoded);
        }
        let body = serde_json::from_slice::<Value>(&raw).unwrap_or(Value::Null);
        let code = body.get("code").and_then(|code| code.as_i64());

        Ok(ApiResponse {
            status,
            code,
            body,
            raw,
            cookies: set_cookies,
        })
    }

    fn ensure_cookie(&self, name: &str, value: String) {
        let mut cookies = self.cookies.lock().expect("cookie lock");
        cookies.entry(name.to_string()).or_insert(value);
    }

    fn base_headers(&self, ua: crate::request::UserAgentKind) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(ua.as_header()));
        headers.insert(REFERER, HeaderValue::from_static(MUSIC_HOST));
        let (os, appver) = self.os_and_appver();
        if let Ok(value) = HeaderValue::from_str(&os) {
            headers.insert("os", value);
        }
        if !appver.is_empty() {
            if let Ok(value) = HeaderValue::from_str(&appver) {
                headers.insert("appver", value);
            }
        }
        headers
    }

    fn attach_headers_and_cookies(
        &self,
        mut req: RequestBuilder,
        headers: HeaderMap,
    ) -> Result<RequestBuilder> {
        req = req.headers(headers);
        let cookie_header = self.cookie_header();
        if !cookie_header.is_empty() {
            req = req.header(COOKIE, cookie_header);
        }
        Ok(req)
    }

    fn cookie_header(&self) -> String {
        let mut cookies = self.cookies.lock().expect("cookie lock").clone();
        let (os, appver) = os_and_appver_from(&cookies);
        cookies.insert("__remember_me".to_string(), "true".to_string());
        cookies.entry("os".to_string()).or_insert(os);
        cookies.entry("appver".to_string()).or_insert(appver);
        cookies
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn os_and_appver(&self) -> (String, String) {
        let cookies = self.cookies.lock().expect("cookie lock");
        os_and_appver_from(&cookies)
    }

    fn eapi_cookie_header(&self) -> BTreeMap<String, Value> {
        let cookies = self.cookies.lock().expect("cookie lock");
        let get = |name: &str, fallback: &str| {
            Value::String(
                cookies
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| fallback.to_string()),
            )
        };
        let mut header = BTreeMap::new();
        header.insert("osver".to_string(), get("osver", "17.4.1"));
        header.insert("deviceId".to_string(), get("deviceId", &self.device_id));
        header.insert("appver".to_string(), get("appver", IOS_APP_VERSION));
        header.insert("versioncode".to_string(), get("versioncode", "140"));
        header.insert("mobilename".to_string(), get("mobilename", ""));
        header.insert(
            "buildver".to_string(),
            get("buildver", &(now_millis() / 1000).to_string()),
        );
        header.insert("resolution".to_string(), get("resolution", "1920x1080"));
        header.insert("__csrf".to_string(), get("__csrf", ""));
        header.insert("os".to_string(), get("os", "ios"));
        header.insert("channel".to_string(), get("channel", ""));
        header.insert(
            "requestId".to_string(),
            Value::String(format!(
                "{}{}",
                now_millis(),
                rand::thread_rng().gen_range(0..1000)
            )),
        );
        if let Some(value) = cookies.get("MUSIC_U") {
            header.insert("MUSIC_U".to_string(), Value::String(value.clone()));
        }
        if let Some(value) = cookies.get("MUSIC_A") {
            header.insert("MUSIC_A".to_string(), Value::String(value.clone()));
        }
        header
    }
}

fn os_and_appver_from(cookies: &HashMap<String, String>) -> (String, String) {
    let os = cookies
        .get("os")
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "ios".to_string());
    let appver = cookies.get("appver").cloned().unwrap_or_else(|| {
        if os == "pc" {
            String::new()
        } else {
            IOS_APP_VERSION.to_string()
        }
    });
    (os, appver)
}

fn add_csrf(data: &mut Value, csrf: &str) {
    if let Value::Object(map) = data {
        map.insert("csrf_token".to_string(), Value::String(csrf.to_string()));
    }
}

pub(crate) fn replace_api_segment(url: &str, replacement: &str) -> String {
    for needle in ["/weapi/", "/eapi/", "/api/"] {
        if let Some(idx) = url.find(needle) {
            return format!(
                "{}{}{}",
                &url[..idx],
                replacement,
                &url[idx + needle.len()..]
            );
        }
    }
    url.to_string()
}

pub(crate) fn eapi_path(url: &str) -> String {
    Url::parse(url)
        .map(|parsed| parsed.path().to_string())
        .unwrap_or_else(|_| url.to_string())
}

fn stringify_json_value(value: Value) -> String {
    match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn collect_set_cookies(headers: &HeaderMap) -> Vec<Cookie> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| Cookie::new(name.trim(), value.trim()))
        .collect()
}

fn maybe_zlib_decode(raw: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(raw);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

fn generate_s_device_id() -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut rng = rand::thread_rng();
    (0..52)
        .map(|_| HEX[rng.gen_range(0..HEX.len())] as char)
        .collect()
}

fn random_alnum(len: usize) -> String {
    const CHARS: &[u8] = b"1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn random_alnum_hex(len: usize) -> String {
    hex::encode(random_alnum(len).as_bytes())
}

fn create_ntes_nuid() -> String {
    let ua = crate::request::UserAgentKind::Pc.as_header();
    let timestamp = now_millis();
    let random: f64 = rand::thread_rng().gen();
    let (client_width, client_height) = browser_client_dimensions(1920, 1080);
    let raw = format!(
        "{timestamp}{}{}{}{}{random}{}:{}",
        MUSIC_HOST, 1920, 1080, ua, client_width, client_height
    );
    format!("{:x}", md5::compute(html_entity_escape_non_ascii(&raw)))
}

fn browser_client_dimensions(screen_width: i32, screen_height: i32) -> (i32, i32) {
    let mut rng = rand::thread_rng();
    let client_height = screen_height - rng.gen_range(90..=150);
    let client_width = screen_width - if rng.gen_bool(0.5) { 17 } else { 0 };
    (client_width, client_height)
}

fn html_entity_escape_non_ascii(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if (ch as u32) > 255 {
            let _ = write!(out, "&#{};", ch as u32);
        } else {
            out.push(ch);
        }
    }
    out
}

fn random_device_id() -> String {
    generate_s_device_id()
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_segment_rewrite_matches_go_behavior() {
        assert_eq!(
            replace_api_segment("https://music.163.com/api/v3/song/detail", "/weapi/"),
            "https://music.163.com/weapi/v3/song/detail"
        );
        assert_eq!(
            replace_api_segment("https://music.163.com/weapi/v3/song/detail", "/api/"),
            "https://music.163.com/api/v3/song/detail"
        );
    }

    #[test]
    fn client_generates_chain_id_with_device_id() {
        let client = NeteaseMusicClient::new().unwrap();
        let chain_id = client.chain_id();
        assert!(chain_id.starts_with("v1_"));
        assert!(chain_id.contains("_web_login_"));
    }

    #[test]
    fn cookie_header_contains_required_defaults() {
        let client = NeteaseMusicClient::new().unwrap();
        let header = client.cookie_header();
        assert!(header.contains("__remember_me=true"));
        assert!(header.contains("sDeviceId="));
        assert!(header.contains("os=ios"));
    }

    #[test]
    fn request_strategy_sets_stable_browser_login_cookies() {
        let client = NeteaseMusicClient::new().unwrap();
        client.apply_request_strategy();

        let nmtid = client.cookie("NMTID").expect("nmtid cookie");
        let nuid = client.cookie("_ntes_nuid").expect("nuid cookie");
        let header = client.cookie_header();

        assert_eq!(client.cookie("os").as_deref(), Some("pc"));
        assert_eq!(client.cookie("appver").as_deref(), Some(""));
        assert_ne!(nmtid, "some_random_id_from_strategy");
        assert_eq!(nmtid.len(), 32);
        assert_eq!(nuid.len(), 32);
        assert!(header.contains("os=pc"));
        assert!(header.contains("NMTID="));
        assert!(header.contains("_ntes_nuid="));
    }

    #[test]
    fn request_strategy_aligns_headers_with_cookie_os() {
        let client = NeteaseMusicClient::new().unwrap();
        client.apply_request_strategy();

        let headers = client.base_headers(crate::request::UserAgentKind::Pc);

        assert_eq!(headers.get("os").unwrap(), "pc");
        assert!(headers.get("appver").is_none());
    }
}
