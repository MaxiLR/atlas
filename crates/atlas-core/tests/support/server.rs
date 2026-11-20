//! A local HTTP server for tests: it answers scripted routes on `127.0.0.1`
//! at a free port and records every request it receives.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use atlas_core::ports::Endpoints;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::Response;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use url::Url;

/// One scripted answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_owned(), content_type.to_owned())],
            body: body.into(),
        }
    }

    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self::new(status, "application/json", body.to_string())
    }

    pub fn text(status: u16, body: &str) -> Self {
        Self::new(status, "text/plain", body)
    }

    /// A server-sent-events stream, sent whole.
    pub fn sse(body: &str) -> Self {
        Self::new(200, "text/event-stream", body)
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

/// A method and path with the replies it gives, in order; the last one
/// repeats once the others are used.
#[derive(Debug, Clone)]
pub struct Route {
    method: Method,
    path: String,
    replies: VecDeque<Reply>,
}

impl Route {
    pub fn new(method: Method, path: &str, replies: impl IntoIterator<Item = Reply>) -> Self {
        let replies: VecDeque<_> = replies.into_iter().collect();
        assert!(!replies.is_empty(), "route {method} {path} has no reply");
        Self {
            method,
            path: path.to_owned(),
            replies,
        }
    }

    pub fn get(path: &str, replies: impl IntoIterator<Item = Reply>) -> Self {
        Self::new(Method::GET, path, replies)
    }

    pub fn post(path: &str, replies: impl IntoIterator<Item = Reply>) -> Self {
        Self::new(Method::POST, path, replies)
    }

    fn next_reply(&mut self) -> Reply {
        if self.replies.len() > 1 {
            self.replies.pop_front().expect("a reply is left")
        } else {
            self.replies[0].clone()
        }
    }
}

/// A request as the server received it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    /// Header names are lower case.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RecordedRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Default)]
struct Script {
    routes: Vec<Route>,
    requests: Vec<RecordedRequest>,
}

/// Stops when dropped.
pub struct TestServer {
    url: Url,
    script: Arc<Mutex<Script>>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl TestServer {
    pub async fn start(routes: impl IntoIterator<Item = Route>) -> Self {
        let script = Arc::new(Mutex::new(Script {
            routes: routes.into_iter().collect(),
            requests: Vec::new(),
        }));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let app = Router::new().fallback(answer).with_state(script.clone());
        let (shutdown, stopped) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .expect("the test server runs");
        });
        Self {
            url: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            script,
            shutdown: Some(shutdown),
        }
    }

    /// `http://127.0.0.1:<port>/`.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Every service the core reaches, served by this server.
    pub fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.url)
    }

    /// The requests received so far, in order.
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.script
            .lock()
            .expect("server lock poisoned")
            .requests
            .clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn answer(
    State(script): State<Arc<Mutex<Script>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut script = script.lock().expect("server lock poisoned");
    script.requests.push(RecordedRequest {
        method: method.to_string(),
        path: uri.path().to_owned(),
        query: uri.query().map(str::to_owned),
        headers: headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
        body: body.to_vec(),
    });
    let reply = script
        .routes
        .iter_mut()
        .find(|route| route.method == method && route.path == uri.path())
        .map(Route::next_reply)
        .unwrap_or_else(|| Reply::text(404, "no scripted route"));
    let mut response =
        Response::builder().status(StatusCode::from_u16(reply.status).expect("a valid status"));
    for (name, value) in &reply.headers {
        response = response.header(name, value);
    }
    response
        .body(Body::from(reply.body))
        .expect("a valid response")
}
