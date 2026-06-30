use netease_music::{NeteaseMusicClient, Result};
use serde_json::json;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

fn serve_once(response_headers: &'static str) -> std::io::Result<(String, mpsc::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr().expect("local addr");
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut buf = [0u8; 8192];
        let mut request = Vec::new();
        let mut content_length = 0usize;

        loop {
            let n = stream.read(&mut buf).expect("read request");
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
            if let Some(header_end) = find_header_end(&request) {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                for line in headers.lines() {
                    if let Some(value) = line.strip_prefix("content-length: ") {
                        content_length = value.parse().expect("content length");
                    }
                    if let Some(value) = line.strip_prefix("Content-Length: ") {
                        content_length = value.parse().expect("content length");
                    }
                }
                let body_len = request.len() - header_end - 4;
                if body_len >= content_length {
                    break;
                }
            }
        }

        tx.send(String::from_utf8_lossy(&request).to_string())
            .expect("send captured request");
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 12\r\n{response_headers}\r\n{{\"code\":200}}"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });

    Ok((format!("http://{addr}"), rx))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|window| window == b"\r\n\r\n")
}

#[test]
fn raw_api_sends_cookie_and_stores_set_cookie() -> Result<()> {
    let Some((base_url, rx)) = optional_server("Set-Cookie: MUSIC_U=test-token; Path=/\r\n")?
    else {
        return Ok(());
    };
    let client = NeteaseMusicClient::builder()
        .cookie("__csrf", "csrf-token")
        .build()?;

    let response = client.call_api(
        &format!("{base_url}/api/sms/captcha/sent"),
        json!({"phone":"1"}),
    )?;

    assert_eq!(response.code, Some(200));
    assert_eq!(client.cookie("MUSIC_U").as_deref(), Some("test-token"));

    let request = rx.recv().expect("captured request");
    assert!(request.starts_with("POST /api/sms/captcha/sent HTTP/1.1"));
    assert!(request.contains("Cookie: "));
    assert!(request.contains("__csrf=csrf-token"));
    assert!(request.contains("phone=1"));
    Ok(())
}

#[test]
fn weapi_request_rewrites_api_path_and_encrypts_form() -> Result<()> {
    let Some((base_url, rx)) = optional_server("")? else {
        return Ok(());
    };
    let client = NeteaseMusicClient::new()?;

    let response = client.call_weapi(
        &format!("{base_url}/api/cloudsearch/pc"),
        json!({"s":"abc"}),
    )?;

    assert_eq!(response.code, Some(200));
    let request = rx.recv().expect("captured request");
    assert!(request.starts_with("POST /weapi/cloudsearch/pc HTTP/1.1"));
    assert!(
        request.contains("content-type: application/x-www-form-urlencoded")
            || request.contains("Content-Type: application/x-www-form-urlencoded")
    );
    assert!(request.contains("params="));
    assert!(request.contains("encSecKey="));
    Ok(())
}

#[test]
fn eapi_request_uses_eapi_path_and_header_cookie() -> Result<()> {
    let Some((base_url, rx)) = optional_server("")? else {
        return Ok(());
    };
    let client = NeteaseMusicClient::new()?;

    let response = client.eapi_with_path(
        &format!("{base_url}/api/content/activity/listen/data/total"),
        "/api/content/activity/listen/data/total",
        json!({}),
    )?;

    assert_eq!(response.code, Some(200));
    let request = rx.recv().expect("captured request");
    assert!(request.starts_with("POST /eapi/content/activity/listen/data/total HTTP/1.1"));
    assert!(request.contains("Cookie: "));
    assert!(request.contains("requestId="));
    assert!(request.contains("appver=3.1.17.204416"));
    assert!(request.contains("params="));
    assert!(!request.contains("encSecKey="));
    assert!(!request.contains("__remember_me=true"));
    Ok(())
}

fn optional_server(
    response_headers: &'static str,
) -> Result<Option<(String, mpsc::Receiver<String>)>> {
    match serve_once(response_headers) {
        Ok(server) => Ok(Some(server)),
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping local socket mock test: {err}");
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}
