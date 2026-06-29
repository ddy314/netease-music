use thiserror::Error;

pub type Result<T> = std::result::Result<T, NeteaseError>;

#[derive(Debug, Error)]
pub enum NeteaseError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("url parse failed: {0}")]
    Url(#[from] url::ParseError),

    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("crypto failed: {0}")]
    Crypto(String),

    #[error("response body read failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid option: {0}")]
    InvalidOption(String),
}
