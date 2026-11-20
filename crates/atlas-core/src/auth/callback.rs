//! The loopback listener the browser returns to. The Codex client allowlists
//! one callback, `http://localhost:1455/auth/callback`, so the listener takes
//! that port or fails; it never falls back to another port and never stops
//! whatever holds it.
//!
//! `localhost` resolves to both `127.0.0.1` and `::1`, and browsers commonly
//! try `::1` first, so the listener takes the port on both loopback
//! addresses: a program holding only `::1` would otherwise receive the
//! authorisation code. Another program on either address fails the sign-in.
//! Only a system without IPv6 loopback is served on `127.0.0.1` alone.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
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
    ipv4: TcpListener,
    /// `None` only where the system has no IPv6 loopback.
    ipv6: Option<TcpListener>,
}

impl CallbackListener {
    /// Listens on `127.0.0.1` and `::1` at `port`. Fails with
    /// [`io::ErrorKind::AddrInUse`] when either is taken, holding neither.
    pub async fn bind(port: u16) -> io::Result<Self> {
        let ipv4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let ipv6 = match TcpListener::bind((Ipv6Addr::LOCALHOST, port)).await {
            Ok(ipv6) => Some(ipv6),
            Err(error) if no_ipv6_loopback(&error) => None,
            // Dropping `ipv4` here releases it.
            Err(error) => return Err(error),
        };
        Ok(Self { ipv4, ipv6 })
    }

    /// The next connection on either loopback address.
    async fn accept(&self) -> io::Result<TcpStream> {
        let accepted = match &self.ipv6 {
            Some(ipv6) => tokio::select! {
                accepted = self.ipv4.accept() => accepted,
                accepted = ipv6.accept() => accepted,
            },
            None => self.ipv4.accept().await,
        };
        accepted.map(|(stream, _)| stream)
    }

    /// Waits for the browser to reach the callback path and returns its
    /// query. Other requests get a 404 and the listener keeps waiting.
    pub async fn wait(self) -> io::Result<HashMap<String, String>> {
        let base = Url::parse("http://localhost/").expect("a valid base URL");
        loop {
            let mut stream = self.accept().await?;
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

/// Whether `error`, binding `::1`, means the system has no IPv6 loopback:
/// the address is not configured or the family is not supported. Any other
/// error, a taken port above all, fails the sign-in.
fn no_ipv6_loopback(error: &io::Error) -> bool {
    // `EAFNOSUPPORT`, which `io::ErrorKind` does not name.
    const FAMILY_NOT_SUPPORTED: i32 = if cfg!(target_os = "linux") { 97 } else { 47 };
    matches!(
        error.kind(),
        io::ErrorKind::AddrNotAvailable | io::ErrorKind::Unsupported
    ) || error.raw_os_error() == Some(FAMILY_NOT_SUPPORTED)
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
