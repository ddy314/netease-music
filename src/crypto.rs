use aes::Aes128;
use base64::{engine::general_purpose, Engine as _};
use cbc::Encryptor as CbcEncryptor;
use cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyInit, KeyIvInit};
use ecb::Encryptor as EcbEncryptor;
use num_bigint::BigUint;
use rand::{rngs::OsRng, Rng};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

use crate::{NeteaseError, Result};

const IV: &[u8; 16] = b"0102030405060708";
const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
const LINUXAPI_KEY: &[u8; 16] = b"rFgB&h#%2?^eDg:Q";
const EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";
const PUBLIC_EXPONENT: u32 = 65_537;
const PUBLIC_MODULUS_HEX: &str = "e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7";
const STD_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

type Aes128CbcEnc = CbcEncryptor<Aes128>;
type Aes128EcbEnc = EcbEncryptor<Aes128>;

fn aes_cbc_encrypt(buffer: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Vec<u8> {
    Aes128CbcEnc::new(key.into(), iv.into()).encrypt_padded_vec_mut::<Pkcs7>(buffer)
}

fn aes_ecb_encrypt(buffer: &[u8], key: &[u8; 16]) -> Vec<u8> {
    Aes128EcbEnc::new(key.into()).encrypt_padded_vec_mut::<Pkcs7>(buffer)
}

fn random_secret_pair() -> ([u8; 16], [u8; 16]) {
    let mut rng = OsRng;
    let mut secret = [0u8; 16];
    let mut reversed = [0u8; 16];
    for idx in 0..16 {
        let value = STD_CHARS[rng.gen_range(0..STD_CHARS.len())];
        secret[idx] = value;
        reversed[15 - idx] = value;
    }
    (secret, reversed)
}

fn rsa_no_padding_encrypt(buffer: &[u8]) -> Result<Vec<u8>> {
    let modulus = BigUint::parse_bytes(PUBLIC_MODULUS_HEX.as_bytes(), 16)
        .ok_or_else(|| NeteaseError::Crypto("invalid public modulus".to_string()))?;
    let exponent = BigUint::from(PUBLIC_EXPONENT);

    let mut padded = Vec::with_capacity(128);
    padded.resize(128 - buffer.len(), 0);
    padded.extend_from_slice(buffer);

    let encrypted = BigUint::from_bytes_be(&padded).modpow(&exponent, &modulus);
    let mut out = encrypted.to_bytes_be();
    if out.len() < 128 {
        let mut full = vec![0u8; 128 - out.len()];
        full.extend_from_slice(&out);
        out = full;
    }
    Ok(out)
}

pub(crate) fn to_json_value(data: BTreeMap<String, Value>) -> Value {
    Value::Object(Map::from_iter(data))
}

pub fn weapi_params(data: &Value) -> Result<BTreeMap<String, String>> {
    let text = serde_json::to_vec(data)?;
    let (secret_key, reversed_secret_key) = random_secret_pair();
    let first = aes_cbc_encrypt(&text, PRESET_KEY, IV);
    let first_b64 = general_purpose::STANDARD.encode(first);
    let second = aes_cbc_encrypt(first_b64.as_bytes(), &reversed_secret_key, IV);
    let enc_sec_key = rsa_no_padding_encrypt(&secret_key)?;

    Ok(BTreeMap::from([
        (
            "params".to_string(),
            general_purpose::STANDARD.encode(second),
        ),
        ("encSecKey".to_string(), hex::encode(enc_sec_key)),
    ]))
}

pub fn linuxapi_params(
    method: &str,
    url: &str,
    params: &Value,
) -> Result<BTreeMap<String, String>> {
    let data = serde_json::json!({
        "method": method,
        "url": url,
        "params": params,
    });
    let encrypted = aes_ecb_encrypt(&serde_json::to_vec(&data)?, LINUXAPI_KEY);
    Ok(BTreeMap::from([(
        "eparams".to_string(),
        hex::encode(encrypted).to_uppercase(),
    )]))
}

pub fn eapi_params(url: &str, data: &Value) -> Result<BTreeMap<String, String>> {
    let text = serde_json::to_string(data)?;
    let message = format!("nobody{url}use{text}md5forencrypt");
    let digest = format!("{:x}", md5::compute(message));
    let payload = format!("{url}-36cd479b6b5-{text}-36cd479b6b5-{digest}");
    let encrypted = aes_ecb_encrypt(payload.as_bytes(), EAPI_KEY);
    Ok(BTreeMap::from([(
        "params".to_string(),
        hex::encode(encrypted).to_uppercase(),
    )]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapi_shape_matches_netease_contract() {
        let params = weapi_params(&serde_json::json!({"s":"hello"})).unwrap();
        assert!(params["params"].len() > 16);
        assert_eq!(params["encSecKey"].len(), 256);
        assert!(params["encSecKey"].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn linuxapi_uses_uppercase_hex_eparams() {
        let params = linuxapi_params(
            "POST",
            "https://music.163.com/api/song/lyric",
            &serde_json::json!({"id":"1"}),
        )
        .unwrap();
        let eparams = &params["eparams"];
        assert!(eparams.len() > 32);
        assert_eq!(eparams, &eparams.to_uppercase());
    }

    #[test]
    fn eapi_uses_uppercase_hex_params() {
        let params = eapi_params(
            "/eapi/search/defaultkeyword/get",
            &serde_json::json!({"header": {}}),
        )
        .unwrap();
        let value = &params["params"];
        assert!(value.len() > 32);
        assert_eq!(value, &value.to_uppercase());
    }
}
