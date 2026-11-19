use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::CoreError;

/// How the candidate rates their own knowledge of the topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum SelfRating {
    NewToThis,
    KnowTheBasics,
    UseItRegularly,
    Expert,
}

/// How far the candidate wants to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Goal {
    Overview,
    WorkingKnowledge,
    Expert,
}

/// A placement question as shown: four options, and "Not sure" added by the
/// interface. It never carries which option is right.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlacementQuestion {
    pub id: Uuid,
    /// 1 for the first question asked.
    pub number: u32,
    pub stem: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum PlacementAnswer {
    /// One of the four options, counted from 0.
    Option {
        index: u32,
    },
    NotSure,
}

/// The step placement is at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum PlacementStep {
    /// Waiting for the self-rating and the goal.
    ChooseRating,
    /// Preparing the questions, before the first one is shown.
    PreparingQuestions,
    Question {
        question: PlacementQuestion,
    },
    BuildingIndex,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlacementState {
    pub step: PlacementStep,
    /// The range of questions placement asks.
    pub min_questions: u32,
    pub max_questions: u32,
    /// Set when the step failed; `RetryPlacementStep` repeats it.
    pub error: Option<CoreError>,
}
