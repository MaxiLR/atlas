//! The loopback listener the browser returns to. The Codex client allowlists
//! one callback, `http://localhost:1455/auth/callback`, so the listener takes
//! that port on `127.0.0.1` or fails; it never falls back to another port and
//! never stops whatever holds it.

use std::collections::HashMap;
use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

const CALLBACK_PATH: &str = "/auth/callback";
/// A request head larger than this is not the callback.
const MAX_HEAD: usize = 16 * 1024;
/// How long a connection may take to send its request head.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

const PAGE: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<title>Atlas</title></head><body><p>Sign-in received. You can close this tab and \
return to Atlas.</p></body></html>";

/// The callback URI registered for the Codex client on `port`.
pub(super) fn redirect_uri(port: u16) -> String {
    format!("http://localhost:{port}{CALLBACK_PATH}")
}

pub(super) struct CallbackListener {
    listener: TcpListener,
}

impl CallbackListener {
    /// Listens on `127.0.0.1` at `port`.
    pub async fn bind(port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        Ok(Self { listener })
    }

    /// Waits for the browser to reach the callback path and returns its
    /// query. Other requests get a 404 and the listener keeps waiting.
    pub async fn wait(self) -> io::Result<HashMap<String, String>> {
        let base = Url::parse("http://localhost/").expect("a valid base URL");
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let Ok(Ok(target)) = tokio::time::timeout(READ_TIMEOUT, read_target(&mut stream)).await
            else {
                continue;
            };
            let Ok(url) = base.join(&target) else {
                let _ = respond(&mut stream, "400 Bad Request", "").await;
                continue;
            };
            if url.path() != CALLBACK_PATH {
                let _ = respond(&mut stream, "404 Not Found", "").await;
                continue;
            }
            let _ = respond(&mut stream, "200 OK", PAGE).await;
            return Ok(url.query_pairs().into_owned().collect());
        }
    }
}

/// Reads a request head and returns the target of a `GET`.
async fn read_target(stream: &mut TcpStream) -> io::Result<String> {
    let mut head = Vec::new();
    let mut chunk = [0_u8; 2048];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        if head.len() > MAX_HEAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "head too large"));
        }
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        head.extend_from_slice(&chunk[..read]);
    }
    let head = String::from_utf8_lossy(&head);
    let mut request_line = head.lines().next().unwrap_or_default().split(' ');
    match (request_line.next(), request_line.next()) {
        (Some("GET"), Some(target)) if target.starts_with('/') => Ok(target.to_owned()),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "not a GET")),
    }
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) -> io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}
