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

        let password = match (params.password, params.md5_password) {
            (Some(password), _) => format!("{:x}", md5::compute(password)),
            (None, Some(md5_password)) => md5_password,
            (None, None) => {
                return Err(NeteaseError::InvalidOption(
                    "password or md5_password is required".to_string(),
                ))
            }
        };

        let mut data = json!({
            "phone": params.phone,
            "countrycode": params.countrycode.unwrap_or_else(|| "86".to_string()),
            "csrf_token": params.csrf_token.unwrap_or_default(),
            "password": password,
            "rememberLogin": "true",
            "type": "1",
            "https": "true",
            "remember": "true",
        });
        if let Some(captcha) = params.captcha {
            data["captcha"] = Value::String(captcha);
        }

        self.call_weapi("https://music.163.com/weapi/login/cellphone", data)
    }

    pub fn login_qr_key(&self) -> Result<(ApiResponse, String)> {
        self.prepare_login_context();

        let response = self.call_weapi(
            "https://music.163.com/weapi/login/qrcode/unikey",
            json!({"type": 1, "noCheckToken": true}),
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
        let qrcode_url = format!(
            "http://music.163.com/login?codekey={}&chainId={}",
            unikey,
            self.chain_id()
        );
        Ok((response, qrcode_url))
    }

    pub fn login_qr_check(&self, params: LoginQrCheckParams) -> Result<ApiResponse> {
        if params.unikey.is_empty() {
            return Err(NeteaseError::InvalidOption(
                "unikey is required".to_string(),
            ));
        }
        self.prepare_login_context();
        self.call_weapi(
            "https://music.163.com/weapi/login/qrcode/client/login",
            json!({"type": 1, "noCheckToken": true, "key": params.unikey}),
        )
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
            return self.call_weapi(
                "https://music.163.com/api/search/voice/get",
                json!({
                    "keyword": params.keywords,
                    "scene": "normal",
                    "limit": limit.to_string(),
                    "offset": offset.to_string(),
                }),
            );
        }

        self.call_weapi(
            "https://music.163.com/api/cloudsearch/pc",
            json!({
                "s": params.keywords,
                "type": search_type,
                "limit": limit.to_string(),
                "offset": offset.to_string(),
            }),
        )
    }

    pub fn search_suggest(&self, params: SearchSuggestParams) -> Result<ApiResponse> {
        let suggest_type = match params.suggest_type.as_deref() {
            Some("mobile") | Some("keyword") => "keyword",
            _ => "web",
        };
        self.call_weapi(
            &format!("https://music.163.com/weapi/search/suggest/{suggest_type}"),
            json!({"s": params.keywords}),
        )
    }

    pub fn search_hot_detail(&self) -> Result<ApiResponse> {
        self.call_weapi("https://music.163.com/weapi/hotsearchlist/get", json!({}))
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
        self.call_linuxapi(
            "POST",
            "https://music.163.com/api/v2/banner/get",
            json!({"clientType": client_type}),
        )
    }

    pub fn personalized(&self, params: PersonalizedParams) -> Result<ApiResponse> {
        self.set_cookie("os", "pc");
        self.call_weapi(
            "https://music.163.com/weapi/personalized/playlist",
            json!({
                "limit": params.limit.unwrap_or(30).to_string(),
                "order": "true",
                "n": "1000",
            }),
        )
    }

    pub fn recommend_songs(&self) -> Result<ApiResponse> {
        self.set_cookie("os", "ios");
        self.call_weapi(
            "https://music.163.com/api/v3/discovery/recommend/songs",
            json!({}),
        )
    }

    pub fn recommend_resource(&self) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/weapi/v1/discovery/recommend/resource",
            json!({}),
        )
    }

    pub fn daily_signin(&self, params: DailySigninParams) -> Result<ApiResponse> {
        self.call_weapi(
            "https://music.163.com/weapi/point/dailyTask",
            json!({"type": params.signin_type.unwrap_or(0).to_string()}),
        )
    }

    pub fn song_url(&self, params: SongUrlParams) -> Result<ApiResponse> {
        self.set_cookie("os", "pc");
        self.call_linuxapi(
            "POST",
            "https://music.163.com/api/song/enhance/player/url",
            json!({
                "ids": format!("[{}]", params.id),
                "br": params.br.unwrap_or(320000).to_string(),
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
        self.call_weapi(
            "https://music.163.com/weapi/song/enhance/player/url/v1",
            data,
        )
    }

    pub fn lyric(&self, params: LyricParams) -> Result<ApiResponse> {
        self.call_linuxapi(
            "POST",
            "https://music.163.com/api/song/lyric",
            json!({
                "id": params.id,
                "lv": params.lv.unwrap_or(-1).to_string(),
                "tv": params.tv.unwrap_or(-1).to_string(),
            }),
        )
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

    pub fn captcha_sent(&self, params: CaptchaParams) -> Result<ApiResponse> {
        self.call_api(
            "https://music.163.com/api/sms/captcha/sent",
            json!({
                "phone": params.phone,
                "ctcode": params.countrycode.unwrap_or_else(|| "86".to_string()),
            }),
        )
    }

    pub fn captcha_verify(&self, params: CaptchaParams) -> Result<ApiResponse> {
        self.call_api(
            "https://music.163.com/api/sms/captcha/verify",
            json!({
                "phone": params.phone,
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
            RequestOptions::new(CryptoMode::Eapi).crypto_url(crypto_path),
        )
    }
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
    fn login_requires_some_password_value() {
        let client = NeteaseMusicClient::new().unwrap();
        let err = client
            .login_cellphone(LoginCellphoneParams {
                phone: "13800000000".to_string(),
                ..Default::default()
            })
            .unwrap_err();
        assert!(err.to_string().contains("password"));
    }
}
