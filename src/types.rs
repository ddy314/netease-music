use bytes::Bytes;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
}

impl Cookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiResponse {
    pub status: u16,
    pub code: Option<i64>,
    pub body: Value,
    pub raw: Bytes,
    pub cookies: Vec<Cookie>,
}
