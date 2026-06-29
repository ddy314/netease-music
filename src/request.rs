#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoMode {
    None,
    Weapi,
    Linuxapi,
    Eapi,
}

#[derive(Debug, Clone)]
pub struct RequestOptions {
    pub crypto: CryptoMode,
    pub user_agent: UserAgentKind,
    pub crypto_url: Option<String>,
}

impl RequestOptions {
    pub fn new(crypto: CryptoMode) -> Self {
        Self {
            crypto,
            user_agent: UserAgentKind::Pc,
            crypto_url: None,
        }
    }

    pub fn mobile(mut self) -> Self {
        self.user_agent = UserAgentKind::Mobile;
        self
    }

    pub fn crypto_url(mut self, url: impl Into<String>) -> Self {
        self.crypto_url = Some(url.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserAgentKind {
    Pc,
    Mobile,
}

impl UserAgentKind {
    pub(crate) fn as_header(self) -> &'static str {
        match self {
            Self::Mobile => "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4.1 Mobile/15E148 Safari/604.1",
            Self::Pc => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0",
        }
    }
}
