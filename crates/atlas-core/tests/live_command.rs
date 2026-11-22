//! The live command (`live::run`): each selected harness task run once on a
//! throwaway course, through `Core::handle`, with a report for the engineer.
//! Exercised here with a scripted `FakeProvider` and, for the plan provider's
//! captured streams, the local test server. Nothing here reaches a real
//! service.

mod support;

use std::path::Path;
use std::sync::Arc;

use atlas_core::live::{
    self, Args, ArgsError, Capture, LiveCore, LiveError, LiveResult, LiveTask, Memory, RecordState,
    Report, Search, Structured, TaskReport, Validity,
};
use atlas_core::ports::{SecretRecord, SecretStore};
use atlas_core::protocol::{CoreError, Model, UsageFailure, UsageState};
use atlas_core::provider::chatgpt::ChatGptProvider;
use atlas_core::provider::{ModelProvider, ProviderError, ProviderEvent, ProviderRequest, Task};
use atlas_core::testing::{FakePorts, FakeProvider, FakeScript, FakeTokenSource};
use futures::StreamExt;
use serde_json::{Value, json};
use support::server::{Reply, Route, TestServer};

/// The plan login's access token in every test that signs in.
const TOKEN: &str = "live-secret-token-4f2a";

/// The first section's record summary; the second section's request must
/// carry it.
const FIRST_SUMMARY: &str = "LIVEFIRSTRECORD the plates ride on the convecting mantle.";

// -- scripted answers -----------------------------------------------------------

fn ok(value: impl ToString) -> Result<String, ProviderError> {
    Ok(value.to_string())
}

fn ladder() -> Value {
    json!({ "concepts": [
        "Earth's layers", "The lithosphere", "Plates", "Mantle convection",
        "Plate boundaries", "Ridge push", "Subduction", "Mountain building"
    ]})
}

fn question() -> Value {
    json!({
        "stem": "A plate moves away from a ridge. What happens at the ridge?",
        "options": [
            { "text": "New crust forms", "misconception": null },
            { "text": "Crust is destroyed", "misconception": "Ridges consume crust" },
            { "text": "Nothing changes", "misconception": "Ridges are static" },
            { "text": "The plate sinks", "misconception": "Ridges are subduction zones" }
        ],
        "key": 0
    })
}

/// Three sections; the second relies on what the first introduces. With
/// `broken`, the first relies on a concept no earlier section introduces.
fn index(broken: bool) -> Value {
    let first_relies: Vec<&str> = if broken { vec!["subduction"] } else { vec![] };
    json!({
        "chapters": [
            { "title": "What moves the plates", "sections": [
                { "title": "What drives the plates", "summary": "The mantle beneath.",
                  "introduces": ["mantle convection"], "relies_on": first_relies },
                { "title": "Ridges", "summary": "Where plates are made.",
                  "introduces": ["ridge push"], "relies_on": ["mantle convection"] }
            ]},
            { "title": "Where plates meet", "sections": [
                { "title": "Trenches", "summary": "Where plates sink.",
                  "introduces": ["subduction"], "relies_on": ["ridge push"] }
            ]}
        ],
        "concepts": [
            { "name": "mantle convection", "rung": null },
            { "name": "ridge push", "rung": null },
            { "name": "subduction", "rung": null }
        ],
        "conventions": [
            { "kind": "notation", "key": "v", "meaning": "plate velocity, in cm per year" },
            { "kind": "terminology", "key": "plate", "meaning": "a rigid piece of lithosphere" }
        ]
    })
}

fn record(summary: &str, notation: Value) -> Value {
    json!({
        "summary": summary,
        "concepts": [{ "name": "mantle convection", "line": "Hot rock rises, cool rock sinks." }],
        "notation": notation,
        "terms": [],
        "examples": [],
        "promises": []
    })
}

fn section(prose: &str, record: &Value) -> String {
    format!("{prose}\n\n```atlas-record\n{record}\n```\n")
}

fn first_section() -> String {
    section(
        "Heat escaping from deep inside the Earth keeps the mantle moving.",
        &record(FIRST_SUMMARY, json!([])),
    )
}

fn second_section() -> String {
    section(
        "At a ridge, hot rock rises and pushes the plates apart.",
        &record(
            "Ridges push the plates apart. New crust forms there.",
            json!([]),
        ),
    )
}

fn explained() -> String {
    section(
        "Think of a pot of soup: the warm soup rises and the cool soup sinks.",
        &record(
            "The mantle moves like soup in a pot. The plates ride on it.",
            json!([]),
        ),
    )
}

fn deep_dives() -> Value {
    let child = |title: &str, concept: &str| {
        json!({
            "title": title, "summary": format!("More on {concept}."),
            "introduces": [{ "name": concept, "rung": null }],
            "relies_on": ["mantle convection"]
        })
    };
    json!({ "children": [
        child("Plumes", "mantle plumes"),
        child("Slab pull", "slab pull"),
        child("Heat flow", "heat flow")
    ]})
}

/// Every task scripted to succeed, on two listed models.
fn succeeding() -> FakeProvider {
    FakeProvider::new(FakeScript::sample())
        .with_models(vec![model("gpt-newest"), model("gpt-older")])
        .answer(Task::DraftLadder, [ok(ladder())])
        .answer(Task::PlacementQuestion, [ok(question())])
        .answer(Task::BuildIndex, [ok(index(false))])
        .answer(
            Task::WriteSection,
            [ok(first_section()), ok(second_section())],
        )
        .answer(Task::ProposeDeepDives, [ok(deep_dives())])
        .answer(Task::ExplainDifferently, [ok(explained())])
        .answer(
            Task::Define,
            [ok("The slow churning of the rock under the plates.")],
        )
        .answer(
            Task::SideChatReply,
            [ok("Picture a pot of soup on a stove.")],
        )
}

fn model(slug: &str) -> Model {
    Model {
        slug: slug.to_owned(),
        display_name: slug.to_uppercase(),
    }
}

fn unsupported(param: &str) -> Result<String, ProviderError> {
    Err(ProviderError::Unsupported {
        param: Some(param.to_owned()),
        request_id: Some("req_live".to_owned()),
    })
}

// -- the harness ----------------------------------------------------------------

/// A Codex login, as the app's sign-in stores it.
fn sign_in(secrets: &dyn SecretStore) {
    let login = json!({
        "client_id": "app_EMoamEEZ73f0CkXaXp7hrann",
        "subject": "user-live",
        "email": "engineer@example.com",
        "access_token": TOKEN,
        "refresh_token": "refresh-token-live",
        "expires_at": "2030-01-01T00:00:00Z",
    });
    secrets
        .write(SecretRecord::PlanLogin, &login.to_string())
        .unwrap();
}

struct Harness {
    _dir: tempfile::TempDir,
    provider: Arc<FakeProvider>,
    core: LiveCore,
}

fn start(provider: FakeProvider, signed_in: bool) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let ports = FakePorts::default();
    if signed_in {
        sign_in(ports.secrets.as_ref());
    }
    let provider = Arc::new(provider);
    let core = LiveCore::new(ports.deps(
        dir.path().join("atlas.sqlite3"),
        provider.clone(),
        Arc::new(atlas_core::testing::RecordingEvents::default()),
    ))
    .expect("the live core starts");
    Harness {
        _dir: dir,
        provider,
        core,
    }
}

impl Harness {
    async fn run(&self, tasks: &[LiveTask]) -> Report {
        live::run(tasks, &self.core, &mut std::io::sink())
            .await
            .expect("the run starts")
    }

    fn requests(&self, task: Task) -> Vec<ProviderRequest> {
        self.provider
            .requests()
            .into_iter()
            .filter(|request| request.task == task)
            .collect()
    }
}

fn line(report: &Report, task: LiveTask) -> &TaskReport {
    report
        .tasks
        .iter()
        .find(|line| line.task == task)
        .unwrap_or_else(|| panic!("no line for {task:?} in {report:#?}"))
}

fn records(line: &TaskReport) -> Vec<(RecordState, usize, usize)> {
    line.memory
        .iter()
        .filter_map(|memory| match memory {
            Memory::Record {
                record,
                rejected,
                input_chars,
                ..
            } => Some((*record, *rejected, *input_chars)),
            _ => None,
        })
        .collect()
}

// -- every task passes ------------------------------------------------------------

#[tokio::test]
async fn with_every_task_succeeding_the_report_lists_eight_passes_with_their_details() {
    let harness = start(succeeding(), true);

    let report = harness.run(&LiveTask::ALL).await;

    assert_eq!(
        report
            .tasks
            .iter()
            .map(|line| line.task)
            .collect::<Vec<_>>(),
        LiveTask::ALL.to_vec()
    );
    for line in &report.tasks {
        assert_eq!(line.result, LiveResult::Passed, "{line:#?}");
        assert_eq!(line.model.as_deref(), Some("gpt-newest"), "{line:#?}");
        assert_eq!(line.repairs, 0, "{line:#?}");
    }
    assert!(!report.failed());

    for task in [
        LiveTask::DraftLadder,
        LiveTask::PlacementQuestion,
        LiveTask::BuildIndex,
        LiveTask::ProposeDeepDives,
    ] {
        assert_eq!(line(&report, task).structured, Some(Structured::Accepted));
        assert_eq!(line(&report, task).search, None);
    }
    for task in [
        LiveTask::WriteSection,
        LiveTask::ExplainDifferently,
        LiveTask::SideChatReply,
    ] {
        assert_eq!(line(&report, task).search, Some(Search::Allowed));
        assert_eq!(line(&report, task).structured, None);
    }

    assert_eq!(
        line(&report, LiveTask::BuildIndex).memory,
        vec![Memory::Map {
            valid: Validity::AtOnce,
            concepts: 3,
            conventions: 2,
        }]
    );
    let sections = records(line(&report, LiveTask::WriteSection));
    assert_eq!(sections.len(), 2, "two sections are written");
    let inputs = harness.requests(Task::WriteSection);
    for ((record, rejected, input_chars), request) in sections.iter().zip(&inputs) {
        assert_eq!(*record, RecordState::Confirmed);
        assert_eq!(*rejected, 0);
        assert_eq!(*input_chars, request.input.chars().count());
    }
    let explained = records(line(&report, LiveTask::ExplainDifferently));
    assert_eq!(explained.len(), 1);
    assert_eq!(explained[0].0, RecordState::Confirmed);
    assert_eq!(
        explained[0].2,
        harness.requests(Task::ExplainDifferently)[0]
            .input
            .chars()
            .count()
    );
    assert_eq!(
        line(&report, LiveTask::ProposeDeepDives).memory,
        vec![Memory::Extension {
            valid: Validity::AtOnce
        }]
    );

    assert_eq!(
        report.models.as_ref().unwrap().models,
        vec![model("gpt-newest"), model("gpt-older")],
        "the models are listed in the order reported"
    );
    // The usage endpoint is a closed port here: the reading fails.
    assert_eq!(
        report.usage,
        Ok(UsageState::FirstReadFailed {
            reason: UsageFailure::Network
        })
    );

    let table = report.table();
    for task in LiveTask::ALL {
        assert!(table.contains(task.name()), "{task:?} missing in\n{table}");
    }
    assert!(table.contains("gpt-newest"), "{table}");
}

#[tokio::test]
async fn each_task_makes_one_request_and_write_section_two() {
    let harness = start(succeeding(), true);

    harness.run(&LiveTask::ALL).await;

    let tasks: Vec<Task> = harness
        .provider
        .requests()
        .iter()
        .map(|request| request.task)
        .collect();
    assert_eq!(
        tasks,
        vec![
            Task::DraftLadder,
            Task::PlacementQuestion,
            Task::BuildIndex,
            Task::WriteSection,
            Task::WriteSection,
            Task::ProposeDeepDives,
            Task::ExplainDifferently,
            Task::Define,
            Task::SideChatReply,
        ]
    );
}

#[tokio::test]
async fn the_second_sections_request_carries_the_first_sections_record() {
    let harness = start(succeeding(), true);

    harness.run(&[LiveTask::WriteSection]).await;

    let requests = harness.requests(Task::WriteSection);
    assert_eq!(requests.len(), 2);
    assert!(
        !requests[0].input.contains(FIRST_SUMMARY),
        "the first request cannot know its own record"
    );
    assert!(
        requests[1].input.contains(FIRST_SUMMARY),
        "{}",
        requests[1].input
    );
}

// -- what the route refused --------------------------------------------------------

#[tokio::test]
async fn a_refused_structured_output_shows_as_fallback_used() {
    let provider = FakeProvider::new(FakeScript::sample()).answer(
        Task::DraftLadder,
        [unsupported("text.format"), ok(ladder())],
    );
    let harness = start(provider, true);

    let report = harness.run(&[LiveTask::DraftLadder]).await;

    let ladder = line(&report, LiveTask::DraftLadder);
    assert_eq!(ladder.result, LiveResult::Passed);
    assert_eq!(ladder.structured, Some(Structured::Fallback));
}

#[tokio::test]
async fn a_refused_search_shows_as_refused() {
    let provider = FakeProvider::new(FakeScript::sample()).answer(
        Task::WriteSection,
        [
            unsupported("tools"),
            ok(first_section()),
            ok(second_section()),
        ],
    );
    let harness = start(provider, true);

    let report = harness.run(&[LiveTask::WriteSection]).await;

    let written = line(&report, LiveTask::WriteSection);
    assert_eq!(written.result, LiveResult::Passed, "{written:#?}");
    assert_eq!(written.search, Some(Search::Refused));
}

#[tokio::test]
async fn a_request_the_route_never_answered_reports_neither_accepted_nor_allowed() {
    let plan_limit = || {
        Err(ProviderError::PlanLimit {
            resets_at: None,
            request_id: None,
        })
    };
    let harness = start(
        FakeProvider::new(FakeScript::sample()).answer(Task::DraftLadder, [plan_limit()]),
        true,
    );

    let report = harness.run(&[LiveTask::DraftLadder]).await;

    let ladder = line(&report, LiveTask::DraftLadder);
    assert!(
        matches!(ladder.result, LiveResult::Failed(_)),
        "{ladder:#?}"
    );
    assert_eq!(ladder.structured, None);

    let harness = start(
        FakeProvider::new(FakeScript::sample()).answer(Task::WriteSection, [plan_limit()]),
        true,
    );

    let report = harness.run(&[LiveTask::WriteSection]).await;

    let written = line(&report, LiveTask::WriteSection);
    assert!(
        matches!(written.result, LiveResult::Failed(_)),
        "{written:#?}"
    );
    assert_eq!(written.search, None);
}

#[tokio::test]
async fn a_plan_limit_fails_its_task_and_the_run_and_skips_what_depends_on_it() {
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::DraftLadder, [ok(ladder())])
        .answer(Task::PlacementQuestion, [ok(question())])
        .answer(Task::BuildIndex, [ok(index(false))])
        .answer(
            Task::WriteSection,
            [Err(ProviderError::PlanLimit {
                resets_at: None,
                request_id: None,
            })],
        );
    let harness = start(provider, true);

    let report = harness.run(&LiveTask::ALL).await;

    for task in [
        LiveTask::DraftLadder,
        LiveTask::PlacementQuestion,
        LiveTask::BuildIndex,
    ] {
        assert_eq!(line(&report, task).result, LiveResult::Passed, "{task:?}");
    }
    assert_eq!(
        line(&report, LiveTask::WriteSection).result,
        LiveResult::Failed(CoreError::PlanLimit { resets_at: None })
    );
    for task in [
        LiveTask::ProposeDeepDives,
        LiveTask::ExplainDifferently,
        LiveTask::Define,
        LiveTask::SideChatReply,
    ] {
        assert_eq!(line(&report, task).result, LiveResult::Skipped, "{task:?}");
    }
    assert!(report.failed());
    for task in [
        Task::ProposeDeepDives,
        Task::ExplainDifferently,
        Task::Define,
        Task::SideChatReply,
    ] {
        assert!(harness.requests(task).is_empty(), "{task:?} was requested");
    }
}

// -- course memory ----------------------------------------------------------------

#[tokio::test]
async fn a_map_needing_its_repair_shows_as_valid_after_repair() {
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::BuildIndex, [ok(index(true))])
        .answer(Task::RepairStructured, [ok(index(false))]);
    let harness = start(provider, true);

    let report = harness.run(&[LiveTask::BuildIndex]).await;

    let built = line(&report, LiveTask::BuildIndex);
    assert_eq!(built.result, LiveResult::Passed, "{built:#?}");
    assert_eq!(built.repairs, 1);
    assert_eq!(
        built.memory,
        vec![Memory::Map {
            valid: Validity::AfterRepair,
            concepts: 3,
            conventions: 2,
        }]
    );
}

#[tokio::test]
async fn a_record_failing_twice_passes_with_an_unconfirmed_record() {
    let no_summary = json!({
        "concepts": [{ "name": "mantle convection", "line": "It moves." }],
        "notation": [], "terms": [], "examples": [], "promises": []
    });
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(
            Task::WriteSection,
            [
                ok(section("The mantle moves.", &no_summary)),
                ok(second_section()),
            ],
        )
        .answer(Task::RepairRecord, [ok(no_summary.clone())]);
    let harness = start(provider, true);

    let report = harness.run(&[LiveTask::WriteSection]).await;

    let written = line(&report, LiveTask::WriteSection);
    assert_eq!(written.result, LiveResult::Passed, "{written:#?}");
    assert_eq!(written.repairs, 1);
    let sections = records(written);
    assert_eq!(sections[0].0, RecordState::Unconfirmed);
    assert_eq!(sections[1].0, RecordState::Confirmed);
}

#[tokio::test]
async fn a_conflicting_convention_shows_as_one_rejected_entry() {
    let redefines = section(
        "Here v stands for the volume of the mantle.",
        &record(
            "The mantle is large. Its volume is v.",
            json!([
                { "key": "v", "meaning": "the volume of the mantle" },
                { "key": "T", "meaning": "the temperature of the mantle" }
            ]),
        ),
    );
    let provider = FakeProvider::new(FakeScript::sample())
        .answer(Task::BuildIndex, [ok(index(false))])
        .answer(Task::WriteSection, [ok(redefines), ok(second_section())]);
    let harness = start(provider, true);

    let report = harness
        .run(&[LiveTask::BuildIndex, LiveTask::WriteSection])
        .await;

    let sections = records(line(&report, LiveTask::WriteSection));
    assert_eq!(sections[0], (RecordState::Confirmed, 1, sections[0].2));
    assert_eq!(sections[1].1, 0);
}

// -- selection --------------------------------------------------------------------

#[tokio::test]
async fn only_the_selected_tasks_make_requests() {
    let harness = start(succeeding(), true);

    let report = harness.run(&[LiveTask::Define]).await;

    assert_eq!(
        report
            .tasks
            .iter()
            .map(|line| line.task)
            .collect::<Vec<_>>(),
        vec![LiveTask::Define]
    );
    assert_eq!(line(&report, LiveTask::Define).result, LiveResult::Passed);
    let tasks: Vec<Task> = harness
        .provider
        .requests()
        .iter()
        .map(|request| request.task)
        .collect();
    assert_eq!(tasks, vec![Task::Define]);
}

fn args(words: &[&str]) -> Result<Args, ArgsError> {
    live::parse_args(words.iter().map(|word| word.to_string()))
}

#[test]
fn task_selects_tasks_and_all_selects_every_one() {
    let parsed = args(&[
        "--task",
        "define",
        "--task",
        "build-index",
        "--model",
        "gpt-older",
        "--capture",
        "/tmp/streams",
    ])
    .unwrap();
    assert_eq!(parsed.tasks, vec![LiveTask::Define, LiveTask::BuildIndex]);
    assert_eq!(parsed.model.as_deref(), Some("gpt-older"));
    assert_eq!(parsed.capture.as_deref(), Some(Path::new("/tmp/streams")));

    let all = args(&["--all"]).unwrap();
    assert_eq!(all.tasks, LiveTask::ALL.to_vec());
    assert_eq!(all.model, None);
    assert_eq!(all.capture, None);

    for task in LiveTask::ALL {
        assert_eq!(args(&["--task", task.name()]).unwrap().tasks, vec![task]);
    }
}

#[test]
fn an_unknown_task_is_rejected_with_the_list_of_names() {
    let error = args(&["--task", "write-poem"]).unwrap_err();

    let message = error.to_string();
    assert!(message.contains("write-poem"), "{message}");
    for task in LiveTask::ALL {
        assert!(message.contains(task.name()), "{message}");
    }
}

#[test]
fn no_task_selected_is_rejected() {
    assert!(args(&[]).is_err());
    assert!(args(&["--model", "gpt-older"]).is_err());
    assert!(args(&["--task"]).is_err());
}

// -- the plan provider ---------------------------------------------------------------

/// A Responses stream that answers with `text`.
fn stream_of(text: &str) -> String {
    let delta = json!({
        "type": "response.output_text.delta", "item_id": "msg_live", "output_index": 0,
        "content_index": 0, "delta": text,
    });
    let completed = json!({
        "type": "response.completed",
        "response": { "id": "resp_live", "status": "completed", "output": [] },
    });
    format!(
        "event: response.output_text.delta\ndata: {delta}\n\n\
         event: response.completed\ndata: {completed}\n\n"
    )
}

fn listed_models() -> Reply {
    Reply::json(
        200,
        &json!({ "models": [
            { "slug": "gpt-newest", "display_name": "GPT Newest", "visibility": "list" }
        ]}),
    )
}

/// The core on the plan provider, against `server`, capturing into `capture`.
#[allow(clippy::field_reassign_with_default)]
fn plan_core(
    dir: &Path,
    server: &TestServer,
    signed_in: bool,
    capture: Option<Arc<Capture>>,
) -> LiveCore {
    let mut ports = FakePorts::default();
    ports.endpoints = server.endpoints();
    if signed_in {
        sign_in(ports.secrets.as_ref());
    }
    let provider = ChatGptProvider::awaiting_login(&ports.endpoints, ports.clock.clone());
    let provider = match &capture {
        Some(capture) => provider.with_capture(capture.clone()),
        None => provider,
    };
    let core = LiveCore::new(ports.deps(
        dir.join("atlas.sqlite3"),
        Arc::new(provider),
        Arc::new(atlas_core::testing::RecordingEvents::default()),
    ))
    .expect("the live core starts");
    match capture {
        Some(capture) => core.with_capture(capture),
        None => core,
    }
}

#[tokio::test]
async fn with_capture_the_streams_are_written_under_the_directory_without_the_token() {
    let ladder_stream = stream_of(&ladder().to_string());
    let server = TestServer::start([
        Route::get("/api/models", [listed_models()]),
        Route::post("/api/responses", [Reply::sse(&ladder_stream)]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let captured = dir.path().join("captured");
    let core = plan_core(
        dir.path(),
        &server,
        true,
        Some(Arc::new(Capture::new(&captured))),
    );

    let report = live::run(&[LiveTask::DraftLadder], &core, &mut std::io::sink())
        .await
        .unwrap();

    assert_eq!(
        line(&report, LiveTask::DraftLadder).result,
        LiveResult::Passed
    );
    let posts: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|request| request.path == "/api/responses")
        .collect();
    assert_eq!(posts.len(), 1, "one request for the one task");
    assert_eq!(
        posts[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );

    let files: Vec<_> = std::fs::read_dir(&captured)
        .expect("the capture directory is written")
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    assert!(
        files[0].starts_with(&captured),
        "{files:?} is outside {captured:?}"
    );
    let written = std::fs::read_to_string(&files[0]).unwrap();
    assert_eq!(written, ladder_stream, "the stream is captured as received");
    assert!(!written.contains(TOKEN));
    assert_eq!(
        report
            .captured
            .as_ref()
            .map(|files| files.as_ref().ok().map(Vec::len)),
        Some(Some(1))
    );
}

#[tokio::test]
async fn the_capture_tap_sees_the_body_only_and_changes_nothing_in_the_stream() {
    let body = stream_of("Hello, world!");
    let server = TestServer::start([Route::post("/api/responses", [Reply::sse(&body)])]).await;
    let dir = tempfile::tempdir().unwrap();
    let captured = dir.path().join("captured");
    let capture = Arc::new(Capture::new(&captured));
    let clock = FakePorts::default().clock;
    let request = || ProviderRequest {
        model: Some("gpt-newest".to_owned()),
        ..ProviderRequest::new(Task::Define, "Define it.", "The term.")
    };
    let events = |provider: ChatGptProvider| async move {
        provider
            .stream(request())
            .await
            .unwrap()
            .collect::<Vec<_>>()
            .await
    };
    let tokens = || Arc::new(FakeTokenSource::new(TOKEN));

    let plain = events(ChatGptProvider::new(
        &server.endpoints(),
        tokens(),
        clock.clone(),
    ))
    .await;
    let tapped = events(
        ChatGptProvider::new(&server.endpoints(), tokens(), clock.clone())
            .with_capture(capture.clone()),
    )
    .await;

    assert_eq!(tapped, plain);
    assert_eq!(
        tapped.last(),
        Some(&Ok::<_, ProviderError>(ProviderEvent::Completed))
    );
    let files = capture.write().unwrap();
    assert_eq!(files.len(), 1);
    let written = std::fs::read_to_string(&files[0]).unwrap();
    assert_eq!(written, body);
    assert!(!written.contains(TOKEN));
    assert!(!written.to_lowercase().contains("bearer"));
}

#[tokio::test]
async fn with_an_empty_secret_store_the_run_stops_before_any_request() {
    let server = TestServer::start([
        Route::get("/api/models", [listed_models()]),
        Route::post("/api/responses", [Reply::sse(&stream_of("{}"))]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let core = plan_core(dir.path(), &server, false, None);

    let result = live::run(&LiveTask::ALL, &core, &mut std::io::sink()).await;

    assert!(matches!(result, Err(LiveError::SignInFirst)), "{result:?}");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("sign in with ChatGPT in the Atlas app first")
    );
    assert!(server.requests().is_empty(), "{:?}", server.requests());
}

#[tokio::test]
async fn with_an_empty_secret_store_the_fake_gets_no_request_either() {
    let harness = start(succeeding(), false);

    let result = live::run(&LiveTask::ALL, &harness.core, &mut std::io::sink()).await;

    assert!(matches!(result, Err(LiveError::SignInFirst)), "{result:?}");
    assert!(harness.provider.requests().is_empty());
}

#[tokio::test]
async fn a_failed_ladder_skips_every_task_after_it() {
    let provider = FakeProvider::new(FakeScript::sample()).answer(
        Task::DraftLadder,
        [Err(ProviderError::PlanLimit {
            resets_at: None,
            request_id: None,
        })],
    );
    let harness = start(provider, true);

    let report = harness.run(&LiveTask::ALL).await;

    assert_eq!(
        line(&report, LiveTask::DraftLadder).result,
        LiveResult::Failed(CoreError::PlanLimit { resets_at: None })
    );
    for task in &LiveTask::ALL[1..] {
        assert_eq!(line(&report, *task).result, LiveResult::Skipped, "{task:?}");
    }
    let tasks: Vec<Task> = harness
        .provider
        .requests()
        .iter()
        .map(|request| request.task)
        .collect();
    assert_eq!(tasks, vec![Task::DraftLadder]);
}

#[tokio::test]
async fn the_model_asked_for_is_the_one_every_request_names() {
    let dir = tempfile::tempdir().unwrap();
    let ports = FakePorts::default();
    sign_in(ports.secrets.as_ref());
    let provider = Arc::new(succeeding());
    let core = LiveCore::new(ports.deps(
        dir.path().join("atlas.sqlite3"),
        provider.clone(),
        Arc::new(atlas_core::testing::RecordingEvents::default()),
    ))
    .unwrap()
    .with_model(Some("gpt-older".to_owned()));

    let report = live::run(&[LiveTask::Define], &core, &mut std::io::sink())
        .await
        .unwrap();

    assert_eq!(
        line(&report, LiveTask::Define).model.as_deref(),
        Some("gpt-older")
    );
    assert_eq!(report.models.unwrap().current.as_deref(), Some("gpt-older"));
    let models: Vec<_> = provider
        .requests()
        .into_iter()
        .map(|request| request.model)
        .collect();
    assert_eq!(models, vec![Some("gpt-older".to_owned())]);
}

#[tokio::test]
async fn the_report_carries_a_usage_reading_taken_with_the_plan_login() {
    let usage = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/usage/five-hour-weekly.json"
    ))
    .unwrap();
    let server = TestServer::start([Route::get(
        "/chatgpt/backend-api/wham/usage",
        [Reply::new(200, "application/json", usage)],
    )])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let ports = FakePorts {
        endpoints: server.endpoints(),
        ..FakePorts::default()
    };
    sign_in(ports.secrets.as_ref());
    let core = LiveCore::new(ports.deps(
        dir.path().join("atlas.sqlite3"),
        Arc::new(succeeding()),
        Arc::new(atlas_core::testing::RecordingEvents::default()),
    ))
    .unwrap();

    let report = live::run(&[LiveTask::Define], &core, &mut std::io::sink())
        .await
        .unwrap();

    match &report.usage {
        Ok(UsageState::Connected { reading }) => assert!(!reading.windows.is_empty()),
        other => panic!("expected a usage reading, got {other:?}"),
    }
    assert!(report.table().contains("5h"), "{}", report.table());
    let reads: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|request| request.path == "/chatgpt/backend-api/wham/usage")
        .collect();
    assert!(!reads.is_empty(), "the usage endpoint was read");
    for read in &reads {
        assert_eq!(
            read.header("authorization"),
            Some(format!("Bearer {TOKEN}").as_str())
        );
    }
}
