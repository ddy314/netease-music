//! Rust client for NetEase Cloud Music APIs.

mod client;
mod crypto;
mod error;
mod ncbl;
mod request;
pub mod services;
mod types;

pub use client::{ClientBuilder, NeteaseMusicClient};
pub use crypto::{eapi_params, linuxapi_params, weapi_params};
pub use error::{NeteaseError, Result};
pub use request::{CryptoMode, RequestOptions};
pub use services::*;
pub use types::{ApiResponse, Cookie};
