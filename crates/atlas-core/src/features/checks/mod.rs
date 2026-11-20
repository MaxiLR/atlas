//! Check questions: whether an answer is right, and the profile update the
//! core computes from it, never the model.
//!
//! The first answer to a question is stored and moves its rung one step on
//! the candidate profile: toward known when right, toward unknown when wrong,
//! recording the misconception of the option chosen. A later answer to the
//! same question returns the stored result and changes nothing.

use std::sync::{Arc, Mutex};

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::questions::KeyedCheckQuestion;
use crate::placement::{SearchAnswer, apply_check};
use crate::protocol::{CheckResult, CoreError};
use crate::store::{CheckAnswerRecord, Store};

pub(crate) struct Feature {
    store: Arc<Store>,
    /// Held from reading a question's stored answer to storing a first one,
    /// so two answers to the same question never both update the profile.
    answering: Mutex<()>,
}

impl Feature {
    pub(crate) fn new(_deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            answering: Mutex::new(()),
        })
    }

    pub(crate) async fn answer(
        &self,
        _cx: &Features,
        version_id: Uuid,
        block: u32,
        question: u32,
        choice: u32,
    ) -> Result<CheckResult, CoreError> {
        let version = self
            .store
            .version(version_id)?
            .ok_or_else(|| not_found(format!("version {version_id}")))?;
        let keyed = version
            .checks
            .iter()
            .find(|checks| checks.block == block)
            .ok_or_else(|| not_found(format!("check block {block} of version {version_id}")))?
            .questions
            .get(question as usize)
            .ok_or_else(|| {
                not_found(format!(
                    "question {question} of block {block} of version {version_id}"
                ))
            })?;
        let _answering = self
            .answering
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(stored) = self.store.check_answer(version_id, block, question)? {
            return Ok(result(keyed, &stored));
        }
        let chosen =
            keyed
                .options
                .get(choice as usize)
                .ok_or_else(|| CoreError::InvalidRequest {
                    message: format!(
                        "option {choice} of a question with {} options",
                        keyed.options.len()
                    ),
                })?;
        let correct = choice == keyed.key;
        let course = self
            .store
            .index_node(version.node_id)?
            .ok_or_else(|| not_found(format!("section {}", version.node_id)))?
            .course_id;
        let mut profile = self
            .store
            .profile(course)?
            .ok_or_else(|| not_found(format!("profile of course {course}")))?;
        let answer = if correct {
            SearchAnswer::Right
        } else {
            SearchAnswer::Wrong {
                misconception: chosen.misconception.clone(),
            }
        };
        apply_check(&mut profile, keyed.rung, &answer).map_err(|error| CoreError::NotFound {
            what: error.to_string(),
        })?;
        let stored = self
            .store
            .add_check_answer(version_id, block, question, choice, correct)?;
        self.store.save_profile(course, &profile)?;
        Ok(result(keyed, &stored))
    }
}

fn not_found(what: String) -> CoreError {
    CoreError::NotFound { what }
}

/// The result of a stored answer, told with the question's key and
/// explanation.
fn result(question: &KeyedCheckQuestion, stored: &CheckAnswerRecord) -> CheckResult {
    CheckResult {
        choice: stored.choice,
        correct: stored.correct,
        key: question.key,
        explanation: question.explanation.clone(),
    }
}
