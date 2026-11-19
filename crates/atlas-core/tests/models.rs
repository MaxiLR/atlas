//! The model picker through the core's request handler: the list the plan
//! reports, the default, the candidate's pick across launches, and the model
//! every generation request carries. Each launch is a new core on the same
//! database file in a temp dir, with a provider whose model list the test
//! scripts.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use atlas_core::Core;
use atlas_core::protocol::{CoreError, CoreEvent, CoreRequest, CoreResponse, Model, ModelList};
use atlas_core::provider::{
    FakeProvider, FakeScript, ModelProvider, ProviderError, ProviderRequest, ProviderStream, Task,
};
use atlas_core::testing::{FakePorts, RecordingEvents};
use tempfile::TempDir;

/// The fake provider's generations, with the model listings scripted in
/// order; the last listing answers every later call.
struct ScriptedPlan {
    generations: FakeProvider,
    listings: Mutex<VecDeque<Result<Vec<Model>, ProviderError>>>,
}

#[async_trait]
impl ModelProvider for ScriptedPlan {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        self.generations.stream(request).await
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        let mut listings = self.listings.lock().unwrap();
        if listings.len() > 1 {
            listings.pop_front().unwrap()
        } else {
            listings.front().cloned().expect("a scripted listing")
        }
    }
}

fn model(slug: &str, display_name: &str) -> Model {
    Model {
        slug: slug.to_owned(),
        display_name: display_name.to_owned(),
    }
}

fn gpt_a() -> Model {
    model("gpt-a", "GPT A")
}

fn gpt_b() -> Model {
    model("gpt-b", "GPT B")
}

fn gpt_c() -> Model {
    model("gpt-c", "GPT C")
}

struct Launch {
    core: Core,
    plan: Arc<ScriptedPlan>,
    events: Arc<RecordingEvents>,
}

/// Starts the core on `database`, the plan answering each model listing in
/// turn with `listings`.
fn launch_with(database: &Path, listings: Vec<Result<Vec<Model>, ProviderError>>) -> Launch {
    let plan = Arc::new(ScriptedPlan {
        generations: FakeProvider::new(FakeScript::sample()),
        listings: Mutex::new(listings.into()),
    });
    let events = Arc::new(RecordingEvents::default());
    let core =
        Core::new(FakePorts::default().deps(database.to_owned(), plan.clone(), events.clone()))
            .expect("core starts");
    Launch { core, plan, events }
}

/// Starts the core on `database` with a plan that lists `models`.
fn launch(database: &Path, models: Vec<Model>) -> Launch {
    launch_with(database, vec![Ok(models)])
}

fn database(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("atlas.sqlite3")
}

impl Launch {
    async fn list(&self) -> ModelList {
        match self.core.handle(CoreRequest::ListModels).await {
            Ok(CoreResponse::Models { list }) => list,
            other => panic!("expected the models, got {other:?}"),
        }
    }

    async fn select(&self, slug: &str) -> Result<ModelList, CoreError> {
        self.core
            .handle(CoreRequest::SelectModel {
                slug: slug.to_owned(),
            })
            .await
            .map(|response| match response {
                CoreResponse::Models { list } => list,
                other => panic!("expected the models, got {other:?}"),
            })
    }

    /// Creates a course and returns the model its index request carried.
    async fn generate(&self) -> Option<String> {
        let before = self.plan.generations.requests().len();
        match self
            .core
            .handle(CoreRequest::CreateCourse {
                topic: "Plate tectonics".to_owned(),
            })
            .await
        {
            Ok(CoreResponse::Course { .. }) => {}
            other => panic!("expected a course, got {other:?}"),
        }
        let requests = self.plan.generations.requests();
        assert_eq!(requests.len(), before + 1);
        assert_eq!(requests[before].task, Task::BuildIndex);
        requests[before].model.clone()
    }
}

fn current(list: &ModelList) -> Option<&str> {
    list.current.as_deref()
}

#[tokio::test]
async fn with_no_pick_the_first_listed_model_is_in_use_and_follows_the_list_at_each_launch() {
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    assert_eq!(current(&first.list().await), Some("gpt-a"));
    drop(first);

    let second = launch(&database(&dir), vec![gpt_c(), gpt_a(), gpt_b()]);

    assert_eq!(current(&second.list().await), Some("gpt-c"));
    assert_eq!(second.generate().await.as_deref(), Some("gpt-c"));
}

#[tokio::test]
async fn a_pick_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    let picked = first.select("gpt-b").await.expect("a listed model");
    assert_eq!(current(&picked), Some("gpt-b"));
    assert_eq!(current(&first.list().await), Some("gpt-b"));
    drop(first);

    let second = launch(&database(&dir), vec![gpt_c(), gpt_a(), gpt_b()]);

    assert_eq!(current(&second.list().await), Some("gpt-b"));
}

#[tokio::test]
async fn a_pick_no_longer_listed_gives_way_to_the_first_listed_and_is_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    first.select("gpt-b").await.expect("a listed model");
    drop(first);

    let without_pick = launch(&database(&dir), vec![gpt_c(), gpt_a()]);
    assert_eq!(current(&without_pick.list().await), Some("gpt-c"));
    assert_eq!(without_pick.generate().await.as_deref(), Some("gpt-c"));
    drop(without_pick);

    // Listed again, the old pick is not restored: it was cleared.
    let listed_again = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    assert_eq!(current(&listed_again.list().await), Some("gpt-a"));
}

#[tokio::test]
async fn the_models_come_in_the_order_reported() {
    let dir = tempfile::tempdir().unwrap();
    let reported = vec![
        model("gpt-z", "Zeta"),
        model("gpt-a", "Alpha"),
        model("gpt-m", "Mu"),
    ];
    let launch = launch(&database(&dir), reported.clone());

    let list = launch.list().await;

    assert_eq!(list.models, reported);
    assert_eq!(
        launch.select("gpt-m").await.expect("a listed model").models,
        reported
    );
}

#[tokio::test]
async fn a_generation_request_built_after_a_pick_carries_that_model() {
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    assert_eq!(first.generate().await.as_deref(), Some("gpt-a"));

    first.select("gpt-b").await.expect("a listed model");

    assert_eq!(first.generate().await.as_deref(), Some("gpt-b"));
    let course = match first.core.handle(CoreRequest::ListCourses).await {
        Ok(CoreResponse::Courses { courses }) => courses[0].id,
        other => panic!("expected courses, got {other:?}"),
    };
    let section = match first
        .core
        .handle(CoreRequest::OpenCourse { course_id: course })
        .await
    {
        Ok(CoreResponse::Course { course }) => course.chapters[0].sections[0].id,
        other => panic!("expected a course, got {other:?}"),
    };
    first
        .core
        .handle(CoreRequest::OpenSection {
            course_id: course,
            section_id: section,
        })
        .await
        .expect("the section is written");
    let requests = first.plan.generations.requests();
    let written = requests.last().expect("a request");
    assert_eq!(written.task, Task::WriteSection);
    assert_eq!(written.model.as_deref(), Some("gpt-b"));
    drop(first);

    let second = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    assert_eq!(second.generate().await.as_deref(), Some("gpt-b"));
}

#[tokio::test]
async fn picking_a_model_reports_the_new_list_as_an_event() {
    let dir = tempfile::tempdir().unwrap();
    let launch = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    launch.list().await;
    launch.events.take();

    launch.select("gpt-b").await.expect("a listed model");

    assert_eq!(
        launch.events.take(),
        [CoreEvent::ModelsChanged {
            list: ModelList {
                models: vec![gpt_a(), gpt_b()],
                current: Some("gpt-b".to_owned()),
            },
        }]
    );
}

#[tokio::test]
async fn a_model_the_plan_does_not_list_cannot_be_picked() {
    let dir = tempfile::tempdir().unwrap();
    let launch = launch(&database(&dir), vec![gpt_a(), gpt_b()]);

    let refused = launch.select("gpt-unlisted").await;

    assert!(
        matches!(&refused, Err(CoreError::NotFound { what }) if what.contains("gpt-unlisted")),
        "expected not found, got {refused:?}"
    );
    assert_eq!(current(&launch.list().await), Some("gpt-a"));
    assert_eq!(launch.events.take(), []);
}

#[tokio::test]
async fn a_failed_listing_reports_its_error_and_the_next_request_lists_again() {
    let dir = tempfile::tempdir().unwrap();
    let launch = launch_with(
        &database(&dir),
        vec![Err(ProviderError::Offline), Ok(vec![gpt_a(), gpt_b()])],
    );

    let failed = launch.core.handle(CoreRequest::ListModels).await;
    assert_eq!(failed, Err(CoreError::Offline));

    let list = launch.list().await;
    assert_eq!(list.models, [gpt_a(), gpt_b()]);
    assert_eq!(current(&list), Some("gpt-a"));
}

#[tokio::test]
async fn a_failed_listing_at_launch_keeps_the_last_pick_for_generations() {
    let dir = tempfile::tempdir().unwrap();
    let first = launch(&database(&dir), vec![gpt_a(), gpt_b()]);
    first.select("gpt-b").await.expect("a listed model");
    drop(first);

    let offline = launch_with(&database(&dir), vec![Err(ProviderError::Offline)]);

    assert_eq!(
        offline.core.handle(CoreRequest::ListModels).await,
        Err(CoreError::Offline)
    );
    assert_eq!(offline.generate().await.as_deref(), Some("gpt-b"));
}
