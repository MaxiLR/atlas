//! Opening a section: its current version from the store, or a generation
//! streamed as events and stored, with its record, once complete
//! ([`generation`]).
//!
//! A section counts as read once a complete version of it has been opened or
//! has finished generating in front of the candidate; it is marked read at
//! that moment and the index change is reported. One generation runs per
//! section at a time: a second open joins it and answers with its outcome,
//! and a stop request ends it at once.

mod generation;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{Notify, watch};
use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::ports::EventSink;
use crate::protocol::{
    CoreError, CoreEvent, SectionVersion, SectionView, VersionStyle, VersionSummary,
};
use crate::provider::ModelProvider;
use crate::store::{NodeKind, NodeRecord, Store, StoredSectionVersion};

/// What a generation, or a request that joined it, answers.
type Outcome = Result<SectionView, CoreError>;

pub(crate) struct Feature {
    store: Arc<Store>,
    provider: Arc<dyn ModelProvider>,
    events: Arc<dyn EventSink>,
    /// The generation running for each section.
    runs: Mutex<HashMap<Uuid, Run>>,
}

/// A generation under way, shared by every request for its section.
#[derive(Clone)]
struct Run {
    stop: Arc<Stop>,
    /// Holds the outcome once the generation has ended.
    done: watch::Receiver<Option<Outcome>>,
}

/// A request to stop a generation. It holds once made, so a stop that comes
/// before the generation waits for it is not lost.
#[derive(Default)]
struct Stop {
    requested: AtomicBool,
    signal: Notify,
}

impl Stop {
    fn request(&self) {
        self.requested.store(true, Ordering::Release);
        self.signal.notify_one();
    }

    /// Resolves once a stop has been requested.
    async fn requested(&self) {
        while !self.requested.load(Ordering::Acquire) {
            self.signal.notified().await;
        }
    }
}

/// The request driving a generation. Dropped, finished or not, it takes the
/// generation off the running ones, so a request waiting on it wakes.
struct Driving<'a> {
    runs: &'a Mutex<HashMap<Uuid, Run>>,
    node: Uuid,
    done: watch::Sender<Option<Outcome>>,
}

impl Driving<'_> {
    fn finish(self, outcome: &Outcome) {
        self.done.send_replace(Some(outcome.clone()));
    }
}

impl Drop for Driving<'_> {
    fn drop(&mut self) {
        lock(self.runs).remove(&self.node);
    }
}

/// How a request for a section's generation goes on.
enum Claim<'a> {
    /// A complete version was stored meanwhile.
    Stored(Box<StoredSectionVersion>),
    /// A generation is running: wait for its outcome.
    Join(Run),
    /// No generation is running: this request drives a new one.
    Drive(Arc<Stop>, Driving<'a>),
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().expect("sections lock poisoned")
}

/// What a request answers when the generation it waited on was stopped or
/// abandoned before it ended.
fn stopped() -> CoreError {
    CoreError::Provider {
        message: "the generation was stopped".to_owned(),
    }
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            provider: deps.provider.clone(),
            events: deps.events.clone(),
            runs: Mutex::default(),
        })
    }

    /// The section's current version from the store, with no request;
    /// otherwise a generation, joined if one is running. Either way the
    /// course now resumes at the section.
    pub(crate) async fn open(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        let node = self.node(course_id, section_id)?;
        self.store.set_last_read(course_id, section_id)?;
        if let Some(version) = self.store.current_version(section_id)? {
            return self.opened(cx, &node, version);
        }
        cx.session.require_plan(cx).await?;
        self.run(cx, node, VersionStyle::Original, None, true).await
    }

    /// Stops the generation running for the section, if one is.
    pub(crate) async fn stop(
        &self,
        _cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<(), CoreError> {
        self.node(course_id, section_id)?;
        if let Some(run) = lock(&self.runs).get(&section_id) {
            run.stop.request();
        }
        Ok(())
    }

    /// Generates the section again, or joins the generation running for it.
    pub(crate) async fn retry(
        &self,
        cx: &Features,
        course_id: Uuid,
        section_id: Uuid,
    ) -> Result<SectionView, CoreError> {
        self.node(course_id, section_id)?;
        self.generate(cx, section_id, VersionStyle::Original, None)
            .await
    }

    /// Generates a new version of the section `node` in `style`, streamed as
    /// events; once complete it is stored with its record, becomes current
    /// and the section is read. `parent` is the version the new one rewrites:
    /// its record goes with the request as the account of that version. A
    /// generation already running for the section is joined instead.
    pub(crate) async fn generate(
        &self,
        cx: &Features,
        node: Uuid,
        style: VersionStyle,
        parent: Option<Uuid>,
    ) -> Result<SectionView, CoreError> {
        let node = self.section_node(node)?;
        self.run(cx, node, style, parent, false).await
    }

    /// Drives a new generation or joins the running one. With
    /// `reuse_stored`, a version stored since the caller looked answers
    /// instead.
    async fn run(
        &self,
        cx: &Features,
        node: NodeRecord,
        style: VersionStyle,
        parent: Option<Uuid>,
        reuse_stored: bool,
    ) -> Outcome {
        match self.claim(node.id, reuse_stored)? {
            Claim::Stored(version) => self.opened(cx, &node, *version),
            Claim::Join(run) => join(run).await,
            Claim::Drive(stop, driving) => {
                let outcome = self.write(cx, &node, style, parent, &stop).await;
                driving.finish(&outcome);
                outcome
            }
        }
    }

    fn claim(&self, node: Uuid, reuse_stored: bool) -> Result<Claim<'_>, CoreError> {
        let mut runs = lock(&self.runs);
        if let Some(run) = runs.get(&node) {
            return Ok(Claim::Join(run.clone()));
        }
        if reuse_stored && let Some(version) = self.store.current_version(node)? {
            return Ok(Claim::Stored(Box::new(version)));
        }
        let stop = Arc::new(Stop::default());
        let (done, receiver) = watch::channel(None);
        runs.insert(
            node,
            Run {
                stop: stop.clone(),
                done: receiver,
            },
        );
        Ok(Claim::Drive(
            stop,
            Driving {
                runs: &self.runs,
                node,
                done,
            },
        ))
    }

    /// A complete version opened: the section is read from now on.
    fn opened(
        &self,
        cx: &Features,
        node: &NodeRecord,
        version: StoredSectionVersion,
    ) -> Result<SectionView, CoreError> {
        self.mark_read(cx, node)?;
        self.view(node, version)
    }

    /// Marks the section read, unless it was, and reports the index change.
    fn mark_read(&self, cx: &Features, node: &NodeRecord) -> Result<(), CoreError> {
        let read = self
            .store
            .index_node(node.id)?
            .is_some_and(|stored| stored.read_at.is_some());
        if read {
            return Ok(());
        }
        self.store.mark_read(node.id)?;
        self.events.emit(CoreEvent::IndexChanged {
            course_id: node.course_id,
            chapters: cx.library.course(node.course_id)?.chapters,
        });
        Ok(())
    }

    /// The section `section_id` of the course `course_id`.
    fn node(&self, course_id: Uuid, section_id: Uuid) -> Result<NodeRecord, CoreError> {
        let node = self.section_node(section_id)?;
        if node.course_id == course_id {
            Ok(node)
        } else {
            Err(not_found(section_id))
        }
    }

    fn section_node(&self, section_id: Uuid) -> Result<NodeRecord, CoreError> {
        self.store
            .index_node(section_id)?
            .filter(|node| node.kind == NodeKind::Section)
            .ok_or_else(|| not_found(section_id))
    }

    fn view(&self, node: &NodeRecord, version: StoredSectionVersion) -> Outcome {
        let versions = self
            .store
            .versions(node.id)?
            .into_iter()
            .map(|version| VersionSummary {
                id: version.id,
                style: version.style,
                created_at: version.created_at,
            })
            .collect();
        Ok(SectionView {
            id: node.id,
            course_id: node.course_id,
            title: node.title.clone(),
            summary: node.line.clone(),
            version: SectionVersion {
                id: version.id,
                style: version.style,
                blocks: version.blocks,
                citations: version.citations,
                sources: version.sources,
                written_without_search: version.written_without_search,
                model: version.model,
                created_at: version.created_at,
            },
            versions,
        })
    }
}

/// Waits for a running generation's outcome.
async fn join(run: Run) -> Outcome {
    let mut done = run.done;
    match done.wait_for(Option::is_some).await {
        Ok(outcome) => outcome.clone().unwrap_or_else(|| Err(stopped())),
        Err(_) => Err(stopped()),
    }
}

fn not_found(section_id: Uuid) -> CoreError {
    CoreError::NotFound {
        what: format!("section {section_id}"),
    }
}
