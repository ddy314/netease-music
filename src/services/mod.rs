use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::CryptoMode;
use crate::{ApiResponse, Cookie, NeteaseError, NeteaseMusicClient, RequestOptions, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginCellphoneParams {
    pub phone: String,
    pub countrycode: Option<String>,
    pub password: Option<String>,
    pub md5_password: Option<String>,
    pub captcha: Option<String>,
    pub csrf_token: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginQrCheckParams {
    pub unikey: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchParams {
    pub keywords: String,
    pub search_type: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchSuggestParams {
    pub keywords: String,
    pub suggest_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PagedParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPlaylistParams {
    pub uid: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonalizedParams {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BannerParams {
    pub client_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongUrlParams {
    pub id: String,
    pub br: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SongQualityLevel {
    Standard,
    #[default]
    Higher,
    Exhigh,
    Lossless,
    Hires,
    #[serde(rename = "jyeffect")]
    JyEffect,
    Sky,
    #[serde(rename = "jymaster")]
    JyMaster,
}

impl SongQualityLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Higher => "higher",
            Self::Exhigh => "exhigh",
            Self::Lossless => "lossless",
            Self::Hires => "hires",
            Self::JyEffect => "jyeffect",
            Self::Sky => "sky",
            Self::JyMaster => "jymaster",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongUrlV1Params {
    pub id: String,
    pub level: Option<SongQualityLevel>,
    pub encode_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricParams {
    pub id: String,
    pub lv: Option<i32>,
    pub tv: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScrobbleParams {
    pub id: String,
    pub sourceid: String,
    pub time: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistDetailParams {
    pub id: String,
    pub s: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistTrackAllParams {
    pub id: String,
    pub s: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongDetailParams {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserDetailParams {
    pub uid: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserRecordParams {
    pub uid: String,
    pub record_type: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentSongRecordParams {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListenDataReportParams {
    pub report_type: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListenDataRealtimeReportParams {
    pub report_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LikeListParams {
    pub uid: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailySigninParams {
    pub signin_type: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptchaParams {
    pub phone: String,
    pub countrycode: Option<String>,
    pub captcha: Option<String>,
}

impl NeteaseMusicClient {
    pub fn login_cellphone(&self, params: LoginCellphoneParams) -> Result<ApiResponse> {
        self.prepare_login_context();
        let mut data = json!({
            "phone": params.phone,
            "countrycode": params.countrycode.unwrap_or_else(|| "86".to_string()),
            "type": "1",
            "https": "true",
            "remember": "true",
        });

        if let Some(captcha) = params.captcha {
            data["captcha"] = Value::String(captcha);
        } else {
            let password = match (params.password, params.md5_password) {
                (Some(password), _) => format!("{:x}", md5::compute(password)),
                (None, Some(md5_password)) => md5_password,
                (None, None) => {
                    return Err(NeteaseError::InvalidOption(
                        "password, md5_password, or captcha is required".to_string(),
                    ))
                }
            };
            data["password"] = Value::String(password);
        }
        if let Some(csrf) = params.csrf_token {
            data["csrf_token"] = Value::String(csrf);
        }

        self.call_weapi("https://music.163.com/api/w/login/cellphone", data)
    }

    pub fn login_qr_key(&self) -> Result<(ApiResponse, String)> {
        self.prepare_login_context();
        let response = self.call_eapi(
            "https://music.163.com/api/login/qrcode/unikey",
            json!({"type": 3}),
        )?;
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
            .unwrap_or_default()
            .to_string();
        Ok((response, self.login_qr_url(&unikey, None)))
    }

    pub fn login_qr_url(&self, key: &str, platform: Option<&str>) -> String {
        let mut url = format!("https://music.163.com/login?codekey={key}");
        if platform == Some("web") {
            url.push_str("&chainId=");
            url.push_str(&self.chain_id());
        }
        url
    }

    pub fn login_qr_check(&self, params: LoginQrCheckParams) -> Result<ApiResponse> {
        if params.unikey.is_empty() {
            return Err(NeteaseError::InvalidOption(
                "unikey is required".to_string(),
            ));
        }
        self.prepare_login_context();
        self.call_eapi(
            "https://music.163.com/api/login/qrcode/client/login",
            json!({"type": 3, "key": params.unikey}),
        )
    }

    pub fn login_status(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/api/w/nuser/account/get", json!({}))
    }

    pub fn login_refresh(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/weapi/login/token/refresh", json!({}))
    }

    pub fn logout(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/weapi/logout", json!({}))
    }

    pub fn search(&self, params: SearchParams) -> Result<ApiResponse> {
        let search_type = params.search_type.unwrap_or_else(|| "1".to_string());
        let limit = params.limit.unwrap_or(30);
        let offset = params.offset.unwrap_or(0);
        if search_type == "2000" {
            return self.call_eapi(
                "https://music.163.com/api/search/voice/get",
                json!({
                    "keyword": params.keywords,
                    "scene": "normal",
                    "limit": limit,
                    "offset": offset,
                }),
            );
        }
        self.call_eapi(
            "https://music.163.com/api/cloudsearch/pc",
            json!({
                "s": params.keywords,
                "type": search_type,
                "limit": limit,
                "offset": offset,
                "total": true,
            }),
        )
    }

    pub fn search_suggest(&self, params: SearchSuggestParams) -> Result<ApiResponse> {
        let suggest_type = match params.suggest_type.as_deref() {
            Some("mobile") | Some("keyword") => "keyword",
            _ => "web",
        };
        self.call_weapi(
            &format!("https://music.163.com/api/search/suggest/{suggest_type}"),
            json!({"s": params.keywords}),
        )
    }

    pub fn search_hot_detail(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/api/hotsearchlist/get", json!({}))
    }

    pub fn account(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/api/nuser/account/get", json!({}))
    }

    pub fn banner(&self, params: BannerParams) -> Result<ApiResponse> {
        let client_type = match params.client_type.as_deref() {
            Some("1") | Some("android") => "android",
            Some("2") | Some("iphone") => "iphone",
            Some("3") | Some("ipad") => "ipad",
            _ => "pc",
        };
        self.call_eapi(
            "https://music.163.com/api/v2/banner/get",
            json!({"clientType": client_type}),
        )
    }

    pub fn personalized(&self, params: PersonalizedParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/personalized/playlist",
            json!({
                "limit": params.limit.unwrap_or(30),
                "total": true,
                "n": 1000,
            }),
        )
    }

    pub fn recommend_songs(&self) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/v3/discovery/recommend/songs",
            json!({}),
        )
    }

    pub fn recommend_resource(&self) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/v1/discovery/recommend/resource",
            json!({}),
        )
    }

    pub fn daily_signin(&self, params: DailySigninParams) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/point/dailyTask",
            json!({"type": params.signin_type.unwrap_or(0)}),
        )
    }

    pub fn song_url(&self, params: SongUrlParams) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/song/enhance/player/url",
            json!({
                "ids": serde_json::to_string(&params.id.split(',').collect::<Vec<_>>())?,
                "br": params.br.unwrap_or(999000),
            }),
        )
    }

    pub fn song_url_v1(&self, params: SongUrlV1Params) -> Result<ApiResponse> {
        let level = params.level.unwrap_or_default();
        let mut data = json!({
            "ids": format!("[{}]", params.id),
            "level": level.as_str(),
            "encodeType": params.encode_type.unwrap_or_else(|| "flac".to_string()),
        });
        if level == SongQualityLevel::Sky {
            data["immerseType"] = Value::String("c51".to_string());
        }
        self.call_eapi("https://music.163.com/api/song/enhance/player/url/v1", data)
    }

    pub fn lyric(&self, params: LyricParams) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/song/lyric",
            json!({
                "id": params.id,
                "lv": params.lv.unwrap_or(-1),
                "tv": params.tv.unwrap_or(-1),
                "rv": -1,
                "kv": -1,
                "_nmclfl": 1,
            }),
        )
    }

    pub fn scrobble(&self, params: ScrobbleParams) -> Result<ApiResponse> {
        match crate::ncbl::report_scrobble_v1(self, &params) {
            Err(NeteaseError::InvalidOption(err)) if err.contains("MUSIC_U") => {
                self.scrobble_weblog(params)
            }
            result => result,
        }
    }

    fn scrobble_weblog(&self, params: ScrobbleParams) -> Result<ApiResponse> {
        self.set_cookie("os", "osx");
        self.set_cookie("appver", "3.1.10.5100");
        self.set_cookie("osver", "15.5");
        self.set_cookie("channel", "netease");

        let startplay = self.request(
            "POST",
            "https://clientlog.music.163.com/api/feedback/weblog",
            json!({"logs": scrobble_logs(&params, "startplay")?}),
            RequestOptions::new(CryptoMode::Eapi).crypto_url("/api/feedback/weblog"),
        )?;
        let play = self.request(
            "POST",
            "https://clientlog.music.163.com/api/feedback/weblog",
            json!({"logs": scrobble_logs(&params, "play")?}),
            RequestOptions::new(CryptoMode::Eapi).crypto_url("/api/feedback/weblog"),
        )?;

        Ok(api_response_with_body(
            200,
            json!({
                "code": 200,
                "data": "success",
                "details": {
                    "startplay": startplay.body,
                    "play": play.body,
                },
            }),
            Vec::new(),
        ))
    }

    pub fn playlist_detail(&self, params: PlaylistDetailParams) -> Result<ApiResponse> {
        self.call_linuxapi(
            "POST",
            "https://music.163.com/weapi/v3/playlist/detail",
            json!({
                "id": params.id,
                "n": "100000",
                "s": params.s.unwrap_or(8).to_string(),
            }),
        )
    }

    pub fn playlist_track_all(&self, params: PlaylistTrackAllParams) -> Result<ApiResponse> {
        let response = self.playlist_detail(PlaylistDetailParams {
            id: params.id,
            s: params.s,
        })?;
        let Some(track_ids) = response
            .body
            .pointer("/playlist/trackIds")
            .and_then(Value::as_array)
        else {
            return Ok(response);
        };
        let ids = track_ids
            .iter()
            .filter_map(|track| track.get("id"))
            .filter_map(|id| {
                id.as_i64()
                    .map(|id| id.to_string())
                    .or_else(|| id.as_str().map(ToOwned::to_owned))
            })
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(response);
        }

        let mut tracks = Vec::new();
        for chunk in ids.chunks(500) {
            let detail = self.song_detail(SongDetailParams {
                ids: chunk.to_vec(),
            })?;
            if let Some(songs) = detail.body.get("songs").and_then(Value::as_array) {
                tracks.extend(songs.iter().cloned());
            }
        }
        let mut body = response.body;
        if let Some(playlist) = body.get_mut("playlist").and_then(Value::as_object_mut) {
            playlist.insert("tracks".to_string(), Value::Array(tracks));
        }
        Ok(api_response_with_body(
            response.status,
            body,
            response.cookies,
        ))
    }

    pub fn song_detail(&self, params: SongDetailParams) -> Result<ApiResponse> {
        let c = params
            .ids
            .iter()
            .map(|id| json!({"id": id}))
            .collect::<Vec<_>>();
        self.call_weapi(
            "https://music.163.com/weapi/v3/song/detail",
            json!({
                "ids": format!("[{}]", params.ids.join(",")),
                "c": serde_json::to_string(&c)?,
            }),
        )
    }

    pub fn user_detail(&self, params: UserDetailParams) -> Result<ApiResponse> {
        self.call_weapi(
            &format!("https://music.163.com/weapi/v1/user/detail/{}", params.uid),
            json!({}),
        )
    }

    pub fn user_record(&self, params: UserRecordParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/weapi/v1/play/record",
            json!({
                "uid": params.uid,
                "type": params.record_type.unwrap_or(1).to_string(),
            }),
        )
    }

    pub fn record_recent_song(&self, params: RecentSongRecordParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/play-record/song/list",
            json!({"limit": params.limit.unwrap_or(100)}),
        )
    }

    pub fn recent_listen_list(&self) -> Result<ApiResponse> {
        self.call_eapi("https://music.163.com/api/pc/recent/listen/list", json!({}))
    }

    pub fn listen_data_report(&self, params: ListenDataReportParams) -> Result<ApiResponse> {
        let mut data = json!({"type": params.report_type.unwrap_or_else(|| "week".to_string())});
        if let Some(end_time) = params.end_time {
            data["endTime"] = Value::String(end_time);
        }
        self.call_eapi(
            "https://music.163.com/api/content/activity/listen/data/report",
            data,
        )
    }

    pub fn listen_data_realtime_report(
        &self,
        params: ListenDataRealtimeReportParams,
    ) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/content/activity/listen/data/realtime/report",
            json!({"type": params.report_type.unwrap_or_else(|| "week".to_string())}),
        )
    }

    pub fn listen_data_total(&self) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/content/activity/listen/data/total",
            json!({}),
        )
    }

    pub fn listen_data_today_song_play_rank(&self) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/content/activity/listen/data/today/song/play/rank",
            json!({}),
        )
    }

    pub fn listen_data_year_report(&self) -> Result<ApiResponse> {
        self.call_eapi(
            "https://music.163.com/api/content/activity/listen/data/year/report",
            json!({}),
        )
    }

    pub fn user_playlist(&self, params: UserPlaylistParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/weapi/user/playlist",
            json!({
                "uid": params.uid,
                "limit": params.limit.unwrap_or(30).to_string(),
                "offset": params.offset.unwrap_or(0).to_string(),
            }),
        )
    }

    pub fn like_list(&self, params: LikeListParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/weapi/song/like/get",
            json!({"uid": params.uid}),
        )
    }

    pub fn register_anonymous(&self) -> Result<ApiResponse> {
        let device_id = self
            .cookie("deviceId")
            .or_else(|| self.cookie("sDeviceId"))
            .unwrap_or_default();
        if !device_id.is_empty() {
            self.set_cookie("deviceId", device_id.clone());
        }
        self.call_weapi(
            "https://music.163.com/api/register/anonimous",
            json!({"username": anonymous_username(&device_id)}),
        )
    }

    pub fn captcha_sent(&self, params: CaptchaParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/sms/captcha/sent",
            json!({
                "cellphone": params.phone,
                "ctcode": params.countrycode.unwrap_or_else(|| "86".to_string()),
                "secrete": "music_middleuser_pclogin",
            }),
        )
    }

    pub fn captcha_verify(&self, params: CaptchaParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/api/sms/captcha/verify",
            json!({
                "cellphone": params.phone,
                "ctcode": params.countrycode.unwrap_or_else(|| "86".to_string()),
                "captcha": params.captcha.unwrap_or_default(),
            }),
        )
    }

    pub fn raw_weapi(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.call_weapi(url, data)
    }

    pub fn raw_eapi(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.call_eapi(url, data)
    }

    pub fn raw_linuxapi(&self, method: &str, url: &str, data: Value) -> Result<ApiResponse> {
        self.call_linuxapi(method, url, data)
    }

    pub fn raw_api(&self, url: &str, data: Value) -> Result<ApiResponse> {
        self.call_api(url, data)
    }

    pub fn eapi_with_path(&self, url: &str, crypto_path: &str, data: Value) -> Result<ApiResponse> {
        self.request(
            "POST",
            url,
            data,
            RequestOptions::new(CryptoMode::Eapi)
                .mobile()
                .crypto_url(crypto_path),
        )
    }
}

fn scrobble_logs(params: &ScrobbleParams, action: &str) -> Result<String> {
    let body = if action == "startplay" {
        json!({
            "id": params.id,
            "type": "song",
            "mainsite": "1",
            "mainsiteWeb": "1",
            "content": format!("id={}", params.sourceid),
        })
    } else {
        json!({
            "download": 0,
            "end": "playend",
            "id": params.id,
            "sourceId": params.sourceid,
            "time": params.time,
            "type": "song",
            "wifi": 0,
            "source": "list",
            "mainsite": "1",
            "mainsiteWeb": "1",
            "content": format!("id={}", params.sourceid),
        })
    };
    Ok(serde_json::to_string(
        &json!([{ "action": action, "json": body }]),
    )?)
}

fn anonymous_username(device_id: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};

    const KEY: &str = "3go8&$8*3*3h0k(2)2";
    let xored = device_id
        .bytes()
        .zip(KEY.bytes().cycle())
        .map(|(left, right)| left ^ right)
        .collect::<Vec<_>>();
    let digest = general_purpose::STANDARD.encode(md5::compute(xored).0);
    general_purpose::STANDARD.encode(format!("{device_id} {digest}"))
}

fn api_response_with_body(status: u16, body: Value, cookies: Vec<Cookie>) -> ApiResponse {
    let raw = serde_json::to_vec(&body).unwrap_or_default();
    let code = body.get("code").and_then(Value::as_i64);
    ApiResponse {
        status,
        code,
        body,
        raw: raw.into(),
        cookies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_quality_defaults_to_higher() {
        assert_eq!(SongQualityLevel::default().as_str(), "higher");
        assert_eq!(SongQualityLevel::Sky.as_str(), "sky");
    }

    #[test]
    fn api_response_with_body_refreshes_raw_and_code() {
        let response = api_response_with_body(200, json!({"code": 200, "data": true}), Vec::new());
        assert_eq!(response.code, Some(200));
        assert_eq!(response.raw.as_ref(), br#"{"code":200,"data":true}"#);
    }

    #[test]
    fn login_requires_some_secret() {
        let client = NeteaseMusicClient::new().unwrap();
        let err = client
            .login_cellphone(LoginCellphoneParams {
                phone: "13800000000".to_string(),
                ..Default::default()
            })
            .unwrap_err();
        assert!(err.to_string().contains("captcha"));
    }

    #[test]
    fn scrobble_logs_match_upstream_payload() {
        let params = ScrobbleParams {
            id: "518066366".to_string(),
            sourceid: "36780169".to_string(),
            time: 291,
        };
        let startplay: Value =
            serde_json::from_str(&scrobble_logs(&params, "startplay").unwrap()).unwrap();
        let play: Value = serde_json::from_str(&scrobble_logs(&params, "play").unwrap()).unwrap();

        assert_eq!(startplay[0]["action"], "startplay");
        assert_eq!(startplay[0]["json"]["id"], "518066366");
        assert_eq!(startplay[0]["json"]["mainsiteWeb"], "1");
        assert_eq!(startplay[0]["json"]["content"], "id=36780169");
        assert_eq!(play[0]["action"], "play");
        assert_eq!(play[0]["json"]["sourceId"], "36780169");
        assert_eq!(play[0]["json"]["time"], 291);
        assert_eq!(play[0]["json"]["mainsite"], "1");
    }
}
