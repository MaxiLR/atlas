//! The Wikipedia summary shown under a definition, stored with its entry.
//!
//! This is the only request Atlas makes to Wikipedia: the REST page summary
//! of the entry's term under the Wikipedia endpoint, unauthenticated, with
//! a user agent naming Atlas. A found article and the absence of one are
//! stored with the entry and never asked for again; a failure is not stored,
//! so a later lookup tries again.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::protocol::{CoreError, GlossaryEntry, WikipediaResult};
use crate::store::Store;

/// Wikimedia asks API clients to name themselves.
const USER_AGENT: &str = concat!(
    "Atlas/",
    env!("CARGO_PKG_VERSION"),
    " (desktop learning app by MaxiLR; looks up the terms a reader defines)"
);

/// How long a lookup may take, connection and answer together.
const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct Feature {
    store: Arc<Store>,
    client: Client,
    /// The REST API base; the summary is `{base}/page/summary/{title}`.
    base: Url,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        let wikipedia = &deps.endpoints.wikipedia;
        if wikipedia.cannot_be_a_base() {
            return Err(CoreError::InvalidRequest {
                message: format!("the Wikipedia endpoint {wikipedia} cannot hold a path"),
            });
        }
        // No cookie store and no default headers: the request carries no
        // credentials. Redirects to the canonical title are followed.
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .build()
            .expect("the HTTP client builds");
        Ok(Self {
            store: store.clone(),
            client,
            base: wikipedia.clone(),
        })
    }

    /// The entry's stored result, or the summary of its term, stored unless
    /// the lookup failed. A lookup that finishes for an entry deleted
    /// meanwhile is dropped: nothing is written and the entry is not found.
    pub(crate) async fn lookup(
        &self,
        _cx: &Features,
        entry_id: Uuid,
    ) -> Result<WikipediaResult, CoreError> {
        let entry = self.entry(entry_id)?;
        if let Some(stored) = entry.wikipedia {
            return Ok(stored);
        }
        let result = match self.summary(&entry.term).await {
            Ok(result) => result,
            Err(error) => {
                self.entry(entry_id)?;
                return Ok(WikipediaResult::Failed { error });
            }
        };
        // Stored only while the entry is live.
        self.store.set_wikipedia(entry_id, &result)?;
        Ok(result)
    }

    fn entry(&self, entry_id: Uuid) -> Result<GlossaryEntry, CoreError> {
        self.store
            .glossary_entry(entry_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("glossary entry {entry_id}"),
            })
    }

    async fn summary(&self, term: &str) -> Result<WikipediaResult, CoreError> {
        let response = self
            .client
            .get(self.summary_url(term))
            .send()
            .await
            .map_err(failure)?;
        match response.status() {
            StatusCode::NOT_FOUND => return Ok(WikipediaResult::NoArticle),
            status if !status.is_success() => return Err(failed()),
            _ => {}
        }
        let summary: Summary = response.json().await.map_err(failure)?;
        summary.into_result()
    }

    /// `{base}/page/summary/{title}`: the title with spaces as underscores,
    /// as Wikipedia writes titles in paths, and percent-encoded as one path
    /// segment.
    fn summary_url(&self, term: &str) -> Url {
        let title = term.trim().replace(' ', "_");
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("checked when the feature was built")
            .pop_if_empty()
            .extend(["page", "summary", &title]);
        url
    }
}

/// The parts of a page summary the lookup keeps.
#[derive(Deserialize)]
struct Summary {
    #[serde(rename = "type")]
    kind: String,
    title: String,
    titles: Option<Titles>,
    #[serde(default)]
    extract: String,
    content_urls: Option<ContentUrls>,
}

#[derive(Deserialize)]
struct Titles {
    normalized: String,
}

#[derive(Deserialize)]
struct ContentUrls {
    desktop: PageUrl,
}

#[derive(Deserialize)]
struct PageUrl {
    page: String,
}

impl Summary {
    fn into_result(self) -> Result<WikipediaResult, CoreError> {
        if self.kind == "disambiguation" {
            return Ok(WikipediaResult::NoArticle);
        }
        let url = self.content_urls.ok_or_else(failed)?.desktop.page;
        // The plain title, which older answers give only with underscores.
        let title = match self.titles {
            Some(titles) => titles.normalized,
            None => self.title.replace('_', " "),
        };
        Ok(WikipediaResult::Found {
            title,
            extract: self.extract,
            url,
        })
    }
}

/// Wikipedia could not be reached in time, or answered with something that
/// is not a summary.
fn failure(error: reqwest::Error) -> CoreError {
    if error.is_connect() || error.is_timeout() {
        CoreError::Offline
    } else {
        failed()
    }
}

/// One attempt was made, and it failed.
fn failed() -> CoreError {
    CoreError::FailedAfterRetries { attempts: 1 }
}
