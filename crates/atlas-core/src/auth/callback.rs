//! The loopback listener the browser returns to:
//! `http://127.0.0.1:<free port>/auth/callback`, never `localhost`.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
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

pub(super) struct CallbackListener {
    listener: TcpListener,
    redirect_uri: Url,
}

impl CallbackListener {
    /// Listens on `127.0.0.1` at a free port.
    pub async fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        let redirect_uri = Url::parse(&format!("http://127.0.0.1:{port}{CALLBACK_PATH}"))
            .expect("a valid loopback URL");
        Ok(Self {
            listener,
            redirect_uri,
        })
    }

    pub fn redirect_uri(&self) -> &Url {
        &self.redirect_uri
    }

    /// Waits for the browser to reach the callback path and returns its
    /// query. Other requests get a 404 and the listener keeps waiting.
    pub async fn wait(self) -> io::Result<HashMap<String, String>> {
        loop {
            let (mut stream, _) = self.listener.accept().await?;
            let Ok(Ok(target)) = tokio::time::timeout(READ_TIMEOUT, read_target(&mut stream)).await
            else {
                continue;
            };
            let Ok(url) = self.redirect_uri.join(&target) else {
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
