//! The Wikipedia summary of a glossary entry's term through the core's
//! request handler, against the local test server, on a database in a temp
//! dir.
//!
//! Entries are laid down through the store's repository API on the same file
//! (defining a term through the core needs a plan login and a model).

mod support;

use std::path::PathBuf;
use std::sync::Arc;

use atlas_core::Core;
use atlas_core::ports::Endpoints;
use atlas_core::protocol::{
    Anchor, Block, BlockContent, CoreError, CoreRequest, CoreResponse, VersionStyle,
    WikipediaResult,
};
use atlas_core::provider::{FakeProvider, FakeScript};
use atlas_core::store::{NewGlossaryEntry, NewSectionVersion, Store};
use atlas_core::testing::{FakePorts, RecordingEvents};
use axum::Router;
use axum::http::StatusCode;
use serde_json::json;
use support::sample_course::sample_course;
use support::server::{Reply, Route, TestServer};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use url::Url;
use uuid::Uuid;

/// A term with a space and non-ASCII letters, and the summary path it is
/// asked for under the base: spaces as underscores, the rest percent-encoded
/// as UTF-8.
const MOHO: &str = "Mohorovičić discontinuity";
const MOHO_PATH: &str = "/wikipedia/page/summary/Mohorovi%C4%8Di%C4%87_discontinuity";

const MOHO_URL: &str = "https://en.wikipedia.org/wiki/Mohorovi%C4%8Di%C4%87_discontinuity";
const MOHO_EXTRACT: &str = "The Mohorovičić discontinuity is the boundary between the crust \
                            and the mantle.";

fn moho_summary() -> serde_json::Value {
    json!({
        "type": "standard",
        "title": "Mohorovičić discontinuity",
        "displaytitle": "<span class=\"mw-page-title-main\">Mohorovičić discontinuity</span>",
        "titles": {
            "canonical": "Mohorovi\u{10d}i\u{107}_discontinuity",
            "normalized": "Mohorovičić discontinuity",
            "display": "<span class=\"mw-page-title-main\">Mohorovičić discontinuity</span>"
        },
        "pageid": 20484,
        "lang": "en",
        "dir": "ltr",
        "description": "Boundary between the Earth's crust and the mantle",
        "content_urls": {
            "desktop": {
                "page": MOHO_URL,
                "revisions": "https://en.wikipedia.org/wiki/Mohorovi%C4%8Di%C4%87_discontinuity?action=history"
            },
            "mobile": {
                "page": "https://en.m.wikipedia.org/wiki/Mohorovi%C4%8Di%C4%87_discontinuity"
            }
        },
        "extract": MOHO_EXTRACT,
        "extract_html": "<p>The <b>Mohorovičić discontinuity</b> is the boundary.</p>"
    })
}

fn moho_found() -> WikipediaResult {
    WikipediaResult::Found {
        title: MOHO.to_owned(),
        extract: MOHO_EXTRACT.to_owned(),
        url: MOHO_URL.to_owned(),
    }
}

fn not_found() -> Reply {
    Reply::json(
        404,
        &json!({
            "type": "https://mediawiki.org/wiki/HyperSwitch/errors/not_found",
            "title": "Not found.",
            "method": "get",
            "detail": "Page or revision not found.",
            "uri": "/en.wikipedia.org/v1/page/summary/Mohorovi%C4%8Di%C4%87_discontinuity"
        }),
    )
}

fn disambiguation() -> Reply {
    Reply::json(
        200,
        &json!({
            "type": "disambiguation",
            "title": "Moho",
            "titles": { "canonical": "Moho", "normalized": "Moho", "display": "Moho" },
            "content_urls": { "desktop": { "page": "https://en.wikipedia.org/wiki/Moho" } },
            "extract": "Moho may refer to:"
        }),
    )
}

struct Harness {
    _dir: TempDir,
    core: Core,
    /// The same database file, for laying entries down and reading them.
    store: Store,
    /// The database file, for reading what is left under the store.
    path: PathBuf,
    course: Uuid,
    version: Uuid,
}

impl Harness {
    fn new(endpoints: Endpoints) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path: PathBuf = dir.path().join("atlas.sqlite3");
        let ports = FakePorts {
            endpoints,
            ..FakePorts::default()
        };
        let provider = Arc::new(FakeProvider::new(FakeScript::sample()));
        let events = Arc::new(RecordingEvents::default());
        let core = Core::new(ports.deps(path.clone(), provider, events)).expect("core starts");
        let store = Store::open(&path, ports.clock.clone()).unwrap();

        let sample = sample_course();
        let course = store.create_course(&sample.topic).unwrap().id;
        store
            .insert_index(course, &sample.index, &sample.map, &[])
            .unwrap();
        let read = &sample.records[0];
        store
            .insert_version(
                &NewSectionVersion {
                    id: read.version,
                    node: read.section,
                    style: VersionStyle::Original,
                    blocks: vec![Block {
                        index: 0,
                        content: BlockContent::Prose {
                            text: format!("Below the crust lies the {MOHO}."),
                        },
                    }],
                    checks: Vec::new(),
                    citations: Vec::new(),
                    sources: Vec::new(),
                    written_without_search: false,
                    record: read.record.clone(),
                    model: Some("gpt-5".to_owned()),
                    prompt: "write-section@1".to_owned(),
                },
                &[],
            )
            .unwrap();
        Self {
            _dir: dir,
            core,
            store,
            path,
            course,
            version: read.version,
        }
    }

    fn define(&self, term: &str) -> Uuid {
        self.store
            .add_glossary_entry(&NewGlossaryEntry {
                course: self.course,
                term: term.to_owned(),
                definition: "The boundary between the crust and the mantle.".to_owned(),
                anchor: Anchor {
                    version_id: self.version,
                    block: 0,
                    quote: term.to_owned(),
                },
            })
            .unwrap()
            .id
    }

    async fn lookup(&self, entry_id: Uuid) -> WikipediaResult {
        match self
            .core
            .handle(CoreRequest::LookupWikipedia { entry_id })
            .await
        {
            Ok(CoreResponse::Wikipedia {
                entry_id: answered,
                result,
            }) => {
                assert_eq!(answered, entry_id, "the answer names the entry asked for");
                result
            }
            other => panic!("expected a Wikipedia answer, got {other:?}"),
        }
    }

    fn stored(&self, entry_id: Uuid) -> Option<WikipediaResult> {
        self.store
            .glossary_entry(entry_id)
            .unwrap()
            .expect("the entry exists")
            .wikipedia
    }
}

#[tokio::test]
async fn a_term_is_requested_at_its_encoded_summary_path_with_a_user_agent_naming_atlas() {
    let server =
        TestServer::start([Route::get(MOHO_PATH, [Reply::json(200, &moho_summary())])]).await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define(MOHO);

    harness.lookup(entry).await;

    let requests = server.requests();
    assert_eq!(requests.len(), 1, "one request: {requests:?}");
    let request = &requests[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, MOHO_PATH);
    let agent = request.header("user-agent").expect("a user agent");
    assert!(
        agent.contains("Atlas"),
        "the user agent names Atlas: {agent}"
    );
}

#[tokio::test]
async fn the_request_carries_no_credentials() {
    let server =
        TestServer::start([Route::get(MOHO_PATH, [Reply::json(200, &moho_summary())])]).await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define(MOHO);

    harness.lookup(entry).await;

    let requests = server.requests();
    assert_eq!(requests.len(), 1, "one request: {requests:?}");
    assert_eq!(requests[0].header("authorization"), None);
    assert_eq!(requests[0].header("cookie"), None);
}

#[tokio::test]
async fn a_summary_returns_title_extract_and_url_and_is_not_requested_again() {
    let server =
        TestServer::start([Route::get(MOHO_PATH, [Reply::json(200, &moho_summary())])]).await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define(MOHO);

    assert_eq!(harness.lookup(entry).await, moho_found());
    assert_eq!(harness.stored(entry), Some(moho_found()));

    assert_eq!(harness.lookup(entry).await, moho_found());
    assert_eq!(
        server.requests().len(),
        1,
        "the second lookup is answered from the store"
    );
}

#[tokio::test]
async fn a_missing_article_is_no_article_and_is_stored() {
    let server = TestServer::start([Route::get(MOHO_PATH, [not_found()])]).await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define(MOHO);

    assert_eq!(harness.lookup(entry).await, WikipediaResult::NoArticle);
    assert_eq!(harness.stored(entry), Some(WikipediaResult::NoArticle));

    assert_eq!(harness.lookup(entry).await, WikipediaResult::NoArticle);
    assert_eq!(
        server.requests().len(),
        1,
        "the second lookup is answered from the store"
    );
}

#[tokio::test]
async fn a_disambiguation_page_is_no_article_and_is_stored() {
    let server = TestServer::start([Route::get(
        "/wikipedia/page/summary/Moho",
        [disambiguation()],
    )])
    .await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define("Moho");

    assert_eq!(harness.lookup(entry).await, WikipediaResult::NoArticle);
    assert_eq!(harness.stored(entry), Some(WikipediaResult::NoArticle));

    assert_eq!(harness.lookup(entry).await, WikipediaResult::NoArticle);
    assert_eq!(
        server.requests().len(),
        1,
        "the second lookup is answered from the store"
    );
}

#[tokio::test]
async fn a_server_error_is_failed_and_stores_nothing_so_a_later_lookup_tries_again() {
    let server = TestServer::start([Route::get(
        MOHO_PATH,
        [
            Reply::text(500, "Internal Server Error"),
            Reply::json(200, &moho_summary()),
        ],
    )])
    .await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define(MOHO);

    let result = harness.lookup(entry).await;
    match &result {
        WikipediaResult::Failed { error } => {
            assert_ne!(
                error,
                &CoreError::Offline,
                "the server answered: {result:?}"
            )
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(harness.stored(entry), None);

    assert_eq!(harness.lookup(entry).await, moho_found());
    assert_eq!(server.requests().len(), 2, "the later lookup asks again");
}

#[tokio::test]
async fn a_refused_connection_is_offline_and_stores_nothing() {
    // The default ports point every service at a closed local port.
    let harness = Harness::new(FakePorts::default().endpoints);
    let entry = harness.define(MOHO);

    assert_eq!(
        harness.lookup(entry).await,
        WikipediaResult::Failed {
            error: CoreError::Offline
        }
    );
    assert_eq!(harness.stored(entry), None);
}

#[tokio::test]
async fn a_redirect_is_followed_to_the_article_it_names() {
    // Wikipedia answers a redirect title with a path relative to the asked one.
    let server = TestServer::start([
        Route::get(
            "/wikipedia/page/summary/Moho",
            [Reply::text(302, "").header("location", "Mohorovi%C4%8Di%C4%87_discontinuity")],
        ),
        Route::get(MOHO_PATH, [Reply::json(200, &moho_summary())]),
    ])
    .await;
    let harness = Harness::new(server.endpoints());
    let entry = harness.define("Moho");

    assert_eq!(harness.lookup(entry).await, moho_found());
    let paths: Vec<String> = server.requests().into_iter().map(|r| r.path).collect();
    assert_eq!(paths, ["/wikipedia/page/summary/Moho", MOHO_PATH]);
}

#[tokio::test]
async fn an_unknown_entry_is_not_found_and_requests_nothing() {
    let server = TestServer::start([]).await;
    let harness = Harness::new(server.endpoints());

    let answer = harness
        .core
        .handle(CoreRequest::LookupWikipedia {
            entry_id: Uuid::new_v4(),
        })
        .await;

    assert!(
        matches!(answer, Err(CoreError::NotFound { .. })),
        "expected not found, got {answer:?}"
    );
    assert!(server.requests().is_empty());
}

// -- a lookup for an entry deleted meanwhile ----------------------------------

/// A Wikipedia that tells when a request arrives and answers it, with the
/// summary of the Mohorovičić discontinuity under `status`, only once
/// released.
struct HeldWikipedia {
    url: Url,
    received: Arc<Notify>,
    release: Arc<Notify>,
    server: JoinHandle<()>,
}

impl HeldWikipedia {
    async fn start(status: u16) -> Self {
        let received = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a free port");
        let address = listener.local_addr().expect("the bound address");
        let app = Router::new().fallback({
            let (received, release) = (received.clone(), release.clone());
            move || async move {
                received.notify_one();
                release.notified().await;
                (
                    StatusCode::from_u16(status).expect("a valid status"),
                    axum::Json(moho_summary()),
                )
            }
        });
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("the held server runs");
        });
        Self {
            url: Url::parse(&format!("http://{address}/")).expect("a valid URL"),
            received,
            release,
            server,
        }
    }

    fn endpoints(&self) -> Endpoints {
        Endpoints::local(&self.url)
    }
}

impl Drop for HeldWikipedia {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn a_lookup_that_finishes_for_an_entry_deleted_meanwhile_is_dropped() {
    // A found article and a failed lookup alike.
    for status in [200, 500] {
        let server = HeldWikipedia::start(status).await;
        let harness = Harness::new(server.endpoints());
        let entry = harness.define(MOHO);

        let (looked_up, deleted) = tokio::join!(
            harness
                .core
                .handle(CoreRequest::LookupWikipedia { entry_id: entry }),
            async {
                server.received.notified().await;
                let deleted = harness
                    .core
                    .handle(CoreRequest::DeleteGlossaryEntry { entry_id: entry })
                    .await;
                server.release.notify_one();
                deleted
            }
        );

        assert_eq!(deleted, Ok(CoreResponse::Done), "status {status}");
        assert!(
            matches!(looked_up, Err(CoreError::NotFound { .. })),
            "status {status}: expected not found, got {looked_up:?}"
        );
        let (term, wikipedia): (String, Option<String>) = rusqlite::Connection::open(&harness.path)
            .unwrap()
            .query_row(
                "SELECT term, wikipedia FROM glossary_entries WHERE id = ?1",
                [entry.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (term.as_str(), wikipedia),
            ("", None),
            "status {status}: nothing is written"
        );
    }
}
