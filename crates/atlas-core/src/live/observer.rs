//! The provider the live command builds its core with: the real one, wrapped
//! so every call is recorded (its task, model, input size, schema, search,
//! text, outcome and time) and so no call is made beyond the run's budget.
//!
//! The budget: only the tasks selected for the run reach the provider, each
//! with one request (two for write section) plus the repairs that follow it.
//! A request the route refused as unsupported (structured output or search)
//! is not counted, since the core asks once more without the capability.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;

use super::LiveTask;
use crate::protocol::Model;
use crate::provider::{
    ModelProvider, ProviderError, ProviderEvent, ProviderRequest, ProviderStream, Task, TokenSource,
};

/// One call the core made, as it went.
#[derive(Debug, Clone)]
pub(super) struct Call {
    /// The live task it was spent on.
    pub(super) live: LiveTask,
    pub(super) task: Task,
    pub(super) model: Option<String>,
    pub(super) input_chars: usize,
    pub(super) schema: bool,
    pub(super) web_search: bool,
    /// The text of the response as it arrived, from its last attempt.
    pub(super) text: String,
    pub(super) outcome: Outcome,
    started: Instant,
    ended: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Streaming,
    Completed,
    Incomplete,
    Failed(ProviderError),
}

impl Call {
    /// A repair request, which follows a task's own request.
    pub(super) fn is_repair(&self) -> bool {
        primary(self.task).is_none()
    }

    /// The route refused a capability of the request, which the core asks
    /// for once more without it.
    pub(super) fn refused_capability(&self) -> bool {
        matches!(
            self.outcome,
            Outcome::Failed(ProviderError::Unsupported { .. })
        )
    }

    /// The route answered the request, so it took what the request asked
    /// for (structured output, search).
    pub(super) fn answered(&self) -> bool {
        matches!(self.outcome, Outcome::Completed | Outcome::Incomplete)
    }

    pub(super) fn took(&self) -> Duration {
        self.ended
            .unwrap_or_else(Instant::now)
            .duration_since(self.started)
    }
}

/// The live task a request of `task` belongs to; none for a repair.
fn primary(task: Task) -> Option<LiveTask> {
    Some(match task {
        Task::DraftLadder => LiveTask::DraftLadder,
        Task::PlacementQuestion => LiveTask::PlacementQuestion,
        Task::BuildIndex => LiveTask::BuildIndex,
        Task::WriteSection => LiveTask::WriteSection,
        Task::ProposeDeepDives => LiveTask::ProposeDeepDives,
        Task::ExplainDifferently => LiveTask::ExplainDifferently,
        Task::Define => LiveTask::Define,
        Task::SideChatReply => LiveTask::SideChatReply,
        Task::RepairStructured | Task::RepairBlock | Task::RepairRecord => return None,
    })
}

/// The requests a task may make, besides its repairs.
fn budget(task: LiveTask) -> usize {
    match task {
        LiveTask::WriteSection => 2,
        _ => 1,
    }
}

#[derive(Default)]
struct State {
    allowed: Vec<LiveTask>,
    calls: Vec<Call>,
}

pub(super) struct Observer {
    inner: Arc<dyn ModelProvider>,
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Observer {
    pub(super) fn new(inner: Arc<dyn ModelProvider>) -> Self {
        Self {
            inner,
            state: Arc::default(),
        }
    }

    /// Lets requests of these tasks through from now on, and no others.
    pub(super) fn allow(&self, tasks: &[LiveTask]) {
        lock(&self.state).allowed = tasks.to_vec();
    }

    /// The calls made so far, in order.
    pub(super) fn calls(&self) -> Vec<Call> {
        lock(&self.state).calls.clone()
    }

    /// The live task `request` is spent on, or why it is refused.
    fn admit(state: &State, request: &ProviderRequest) -> Result<LiveTask, ProviderError> {
        let refused = |why: String| ProviderError::failed(format!("the live command {why}"));
        let live = match primary(request.task) {
            Some(live) => live,
            None => state
                .calls
                .iter()
                .rev()
                .find(|call| !call.is_repair())
                .map(|call| call.live)
                .ok_or_else(|| refused("makes no repair before a task's request".to_owned()))?,
        };
        if !state.allowed.contains(&live) {
            return Err(refused(format!("does not run {} in this run", live.name())));
        }
        if primary(request.task).is_some() {
            let spent = state
                .calls
                .iter()
                .filter(|call| call.live == live && !call.is_repair() && !call.refused_capability())
                .count();
            if spent >= budget(live) {
                return Err(refused(format!(
                    "has made the requests {} may make",
                    live.name()
                )));
            }
        }
        Ok(live)
    }
}

/// Records what one call's stream yields.
fn follow(state: &Mutex<State>, index: usize, event: &Result<ProviderEvent, ProviderError>) {
    let mut state = lock(state);
    let call = &mut state.calls[index];
    let outcome = match event {
        Ok(ProviderEvent::TextDelta(text)) => {
            call.text.push_str(text);
            return;
        }
        Ok(ProviderEvent::Retrying { .. }) => {
            call.text.clear();
            return;
        }
        Ok(ProviderEvent::Searching | ProviderEvent::Citation(_) | ProviderEvent::Usage(_)) => {
            return;
        }
        Ok(ProviderEvent::Completed) => Outcome::Completed,
        Ok(ProviderEvent::Incomplete { .. }) => Outcome::Incomplete,
        Err(error) => Outcome::Failed(error.clone()),
    };
    call.outcome = outcome;
    call.ended = Some(Instant::now());
}

#[async_trait]
impl ModelProvider for Observer {
    async fn stream(&self, request: ProviderRequest) -> Result<ProviderStream, ProviderError> {
        let index = {
            let mut state = lock(&self.state);
            let live = Self::admit(&state, &request)?;
            state.calls.push(Call {
                live,
                task: request.task,
                model: request.model.clone(),
                input_chars: request.input.chars().count(),
                schema: request.schema.is_some(),
                web_search: request.web_search,
                text: String::new(),
                outcome: Outcome::Streaming,
                started: Instant::now(),
                ended: None,
            });
            state.calls.len() - 1
        };
        match self.inner.stream(request).await {
            Ok(stream) => {
                let state = self.state.clone();
                Ok(Box::pin(stream.inspect(move |event| {
                    follow(&state, index, event);
                })))
            }
            Err(error) => {
                follow(&self.state, index, &Err(error.clone()));
                Err(error)
            }
        }
    }

    async fn list_models(&self) -> Result<Vec<Model>, ProviderError> {
        self.inner.list_models().await
    }

    fn use_plan_login(&self, tokens: Arc<dyn TokenSource>) -> bool {
        self.inner.use_plan_login(tokens)
    }
}
