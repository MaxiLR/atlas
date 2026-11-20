//! Starting a course and placing the candidate on its concept ladder before
//! the index is built (ADR-0006).
//!
//! A course starts in the placement stage. The self-rating and the goal open
//! placement: the ladder is drafted, then one question at a time is written
//! on the rung the search picks. Each answer is stored; once the search is
//! finished the profile is stored and the index is built. Everything is
//! stored as it happens, so where placement stands is read back from the
//! store and a course left half-way resumes where it was. The ladder, the
//! keys and the misconception labels never leave the core.

mod tasks;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use uuid::Uuid;

use super::Features;
use crate::CoreDeps;
use crate::domain::ladder::Ladder;
use crate::domain::questions::KeyedPlacementQuestion;
use crate::harness::structured::{StructuredError, StructuredRunner};
use crate::placement::{MAX_QUESTIONS, MIN_QUESTIONS, SearchAnswer, next_rung, profile};
use crate::ports::EventSink;
use crate::protocol::{
    CoreError, CoreEvent, CourseStage, CourseView, Goal, PlacementAnswer, PlacementQuestion,
    PlacementState, PlacementStep, SelfRating,
};
use crate::store::{CourseRecord, PlacementQuestionRecord, Store};

pub(crate) struct Feature {
    store: Arc<Store>,
    events: Arc<dyn EventSink>,
    runner: StructuredRunner,
    /// Per course, a step running or the error of the step that failed, in
    /// this session.
    status: Mutex<HashMap<Uuid, Status>>,
}

#[derive(Debug, Clone)]
enum Status {
    Running,
    Failed(CoreError),
}

/// Where a course's placement stands, as the store tells it.
enum Position {
    ChooseRating,
    /// The self-rating is in; the ladder is not drafted.
    DraftLadder(Inputs),
    /// Every question written is answered and the search wants another.
    WriteQuestion(Inputs, Ladder, Vec<PlacementQuestionRecord>, usize),
    /// A question waits for its answer.
    Question(PlacementQuestionRecord),
    /// The search is finished; the index is not built.
    BuildIndex(Inputs, Ladder, Vec<SearchAnswer>),
    /// The index is built.
    Done,
}

/// What the candidate said before the questions.
#[derive(Clone)]
struct Inputs {
    topic: String,
    rating: SelfRating,
    goal: Goal,
}

/// The step the interface was last told about during one request, so a
/// step is announced once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Announced {
    Nothing,
    Preparing,
}

impl Feature {
    pub(crate) fn new(deps: &CoreDeps, store: &Arc<Store>) -> Result<Self, CoreError> {
        Ok(Self {
            store: store.clone(),
            events: deps.events.clone(),
            runner: StructuredRunner::new(deps.provider.clone()),
            status: Mutex::new(HashMap::new()),
        })
    }

    /// Stores a course on `topic`, typed in any language, in the placement
    /// stage. No request is made.
    pub(crate) async fn create_course(
        &self,
        cx: &Features,
        topic: &str,
    ) -> Result<CourseView, CoreError> {
        let topic = topic.trim();
        if topic.is_empty() {
            return Err(CoreError::InvalidRequest {
                message: "the topic is empty".to_owned(),
            });
        }
        let course = self.store.create_course(topic)?;
        let mut view = cx.library.course(course.id)?;
        view.placement = self.state(course.id)?;
        Ok(view)
    }

    /// Where placement stands for a course in the placement stage; `None`
    /// once its index is built.
    pub(crate) fn state(&self, course_id: Uuid) -> Result<Option<PlacementState>, CoreError> {
        let position = self.position(course_id)?;
        Ok(match position {
            Position::Done => None,
            position => Some(self.view(course_id, &position)),
        })
    }

    pub(crate) async fn submit_self_rating(
        &self,
        cx: &Features,
        course_id: Uuid,
        rating: SelfRating,
        goal: Goal,
    ) -> Result<PlacementState, CoreError> {
        match self.position(course_id)? {
            Position::ChooseRating => {}
            Position::Done => return Err(not_in_placement(course_id)),
            _ => {
                return Err(CoreError::InvalidRequest {
                    message: "the self-rating is already chosen".to_owned(),
                });
            }
        }
        self.store.set_self_rating(course_id, rating, goal)?;
        Ok(self.advance(cx, course_id, Announced::Nothing).await)
    }

    /// Stores the answer to the question shown, without saying whether it was
    /// right, and goes on to the next question or, once the search is
    /// finished, the profile and the index.
    pub(crate) async fn answer(
        &self,
        cx: &Features,
        course_id: Uuid,
        question_id: Uuid,
        answer: PlacementAnswer,
    ) -> Result<PlacementState, CoreError> {
        let shown = match self.position(course_id)? {
            Position::Question(record) if record.question.id == question_id => record,
            Position::Done => return Err(not_in_placement(course_id)),
            _ => {
                return Err(CoreError::InvalidRequest {
                    message: format!("question {question_id} is not the one shown"),
                });
            }
        };
        if let PlacementAnswer::Option { index } = answer
            && index as usize >= shown.question.options.len()
        {
            return Err(CoreError::InvalidRequest {
                message: format!("the question has no option {index}"),
            });
        }
        self.store.answer_placement_question(question_id, answer)?;
        Ok(self.advance(cx, course_id, Announced::Preparing).await)
    }

    /// Repeats the step that failed, and that step only. A step that has not
    /// failed is left as it is.
    pub(crate) async fn retry_step(
        &self,
        cx: &Features,
        course_id: Uuid,
    ) -> Result<PlacementState, CoreError> {
        let position = self.position(course_id)?;
        let pending = matches!(
            position,
            Position::DraftLadder(..) | Position::WriteQuestion(..) | Position::BuildIndex(..)
        );
        if matches!(position, Position::Done) {
            return Err(not_in_placement(course_id));
        }
        if !pending || matches!(self.status_of(course_id), Some(Status::Running)) {
            return Ok(self.view(course_id, &position));
        }
        Ok(self.advance(cx, course_id, Announced::Nothing).await)
    }

    /// Runs the steps due until a question waits for its answer, placement
    /// is done, or a step fails, and reports each step as it starts. A failed
    /// step is reported with its error and kept for `RetryPlacementStep`.
    async fn advance(
        &self,
        cx: &Features,
        course_id: Uuid,
        mut announced: Announced,
    ) -> PlacementState {
        self.set_status(course_id, Some(Status::Running));
        let error = loop {
            let position = match self.position(course_id) {
                Ok(position) => position,
                Err(error) => break error,
            };
            let step = match position {
                Position::DraftLadder(inputs) => {
                    self.announce_preparing(course_id, &mut announced);
                    self.draft_ladder(cx, course_id, &inputs).await
                }
                Position::WriteQuestion(inputs, ladder, asked, rung) => {
                    self.announce_preparing(course_id, &mut announced);
                    self.write_question(cx, course_id, &inputs, &ladder, &asked, rung)
                        .await
                }
                Position::BuildIndex(inputs, ladder, answers) => {
                    self.emit(course_id, step_state(PlacementStep::BuildingIndex, None));
                    self.build_index(cx, course_id, &inputs, &ladder, &answers)
                        .await
                }
                position @ (Position::Question(_) | Position::Done | Position::ChooseRating) => {
                    self.set_status(course_id, None);
                    let state = match position {
                        Position::Done => step_state(PlacementStep::Done, None),
                        position => self.view(course_id, &position),
                    };
                    self.emit(course_id, state.clone());
                    return state;
                }
            };
            if let Err(error) = step {
                break error;
            }
        };
        self.set_status(course_id, Some(Status::Failed(error.clone())));
        let state = match self.position(course_id) {
            Ok(position) => self.view(course_id, &position),
            Err(_) => step_state(PlacementStep::PreparingQuestions, Some(error)),
        };
        self.emit(course_id, state.clone());
        state
    }

    fn announce_preparing(&self, course_id: Uuid, announced: &mut Announced) {
        if *announced != Announced::Preparing {
            *announced = Announced::Preparing;
            self.emit(
                course_id,
                step_state(PlacementStep::PreparingQuestions, None),
            );
        }
    }

    async fn draft_ladder(
        &self,
        cx: &Features,
        course_id: Uuid,
        inputs: &Inputs,
    ) -> Result<(), CoreError> {
        let model = cx.models.current(cx).await?;
        let draft = self
            .runner
            .run(
                &tasks::DraftLadder,
                model.as_deref(),
                &tasks::ladder_input(&inputs.topic),
            )
            .await
            .map_err(|error| task_error(error, "concept ladder"))?;
        self.store.store_ladder(course_id, &draft.into_ladder())?;
        Ok(())
    }

    async fn write_question(
        &self,
        cx: &Features,
        course_id: Uuid,
        inputs: &Inputs,
        ladder: &Ladder,
        asked: &[PlacementQuestionRecord],
        rung: usize,
    ) -> Result<(), CoreError> {
        let stems: Vec<&str> = asked
            .iter()
            .map(|record| record.question.stem.as_str())
            .collect();
        let input = tasks::question_input(
            &inputs.topic,
            inputs.goal,
            &ladder.rungs[rung],
            ladder.rungs.len(),
            &stems,
        );
        let model = cx.models.current(cx).await?;
        let draft = self
            .runner
            .run(&tasks::PlacementQuestion, model.as_deref(), &input)
            .await
            .map_err(|error| task_error(error, "placement question"))?;
        let question = draft.into_question(ladder.rungs[rung].id, rand::random::<u8>().into());
        self.store.add_placement_question(course_id, &question)?;
        Ok(())
    }

    async fn build_index(
        &self,
        cx: &Features,
        course_id: Uuid,
        inputs: &Inputs,
        ladder: &Ladder,
        answers: &[SearchAnswer],
    ) -> Result<(), CoreError> {
        let profile = profile(ladder, inputs.rating, inputs.goal, answers);
        self.store.save_profile(course_id, &profile)?;
        cx.index.build(cx, course_id).await
    }

    /// Reads where placement stands from the store.
    fn position(&self, course_id: Uuid) -> Result<Position, CoreError> {
        let course = self
            .store
            .course_record(course_id)?
            .ok_or_else(|| CoreError::NotFound {
                what: format!("course {course_id}"),
            })?;
        if course.stage == CourseStage::Ready {
            return Ok(Position::Done);
        }
        let Some(inputs) = inputs(&course) else {
            return Ok(Position::ChooseRating);
        };
        let Some(ladder) = self.store.ladder(course_id)? else {
            return Ok(Position::DraftLadder(inputs));
        };
        let asked = self.store.placement_questions(course_id)?;
        if let Some(waiting) = asked.iter().find(|record| record.answer.is_none()) {
            return Ok(Position::Question(waiting.clone()));
        }
        let answers: Vec<SearchAnswer> = asked
            .iter()
            .filter_map(|record| Some(search_answer(&record.question, record.answer?)))
            .collect();
        Ok(
            match next_rung(ladder.rungs.len(), inputs.rating, &answers) {
                Some(rung) => Position::WriteQuestion(inputs, ladder, asked, rung),
                None => Position::BuildIndex(inputs, ladder, answers),
            },
        )
    }

    /// The state the interface is shown for `position`. A step due but not
    /// running carries the error it failed with, or, when it failed in an
    /// earlier session, an error saying it stopped, so it can be retried.
    fn view(&self, course_id: Uuid, position: &Position) -> PlacementState {
        let pending = |step| {
            let error = match self.status_of(course_id) {
                Some(Status::Running) => None,
                Some(Status::Failed(error)) => Some(error),
                None => Some(CoreError::Provider {
                    message: "this placement step stopped before it finished".to_owned(),
                }),
            };
            step_state(step, error)
        };
        match position {
            Position::ChooseRating => step_state(PlacementStep::ChooseRating, None),
            Position::DraftLadder(..) | Position::WriteQuestion(..) => {
                pending(PlacementStep::PreparingQuestions)
            }
            Position::Question(record) => step_state(
                PlacementStep::Question {
                    question: shown(record),
                },
                None,
            ),
            Position::BuildIndex(..) => pending(PlacementStep::BuildingIndex),
            Position::Done => step_state(PlacementStep::Done, None),
        }
    }

    fn emit(&self, course_id: Uuid, placement: PlacementState) {
        self.events.emit(CoreEvent::PlacementProgress {
            course_id,
            placement,
        });
    }

    fn statuses(&self) -> MutexGuard<'_, HashMap<Uuid, Status>> {
        self.status.lock().expect("placement status lock poisoned")
    }

    fn status_of(&self, course_id: Uuid) -> Option<Status> {
        self.statuses().get(&course_id).cloned()
    }

    fn set_status(&self, course_id: Uuid, status: Option<Status>) {
        let mut statuses = self.statuses();
        match status {
            Some(status) => statuses.insert(course_id, status),
            None => statuses.remove(&course_id),
        };
    }
}

fn inputs(course: &CourseRecord) -> Option<Inputs> {
    Some(Inputs {
        topic: course.topic.clone(),
        rating: course.self_rating?,
        goal: course.goal?,
    })
}

fn step_state(step: PlacementStep, error: Option<CoreError>) -> PlacementState {
    PlacementState {
        step,
        min_questions: MIN_QUESTIONS as u32,
        max_questions: MAX_QUESTIONS as u32,
        error,
    }
}

/// The question as shown: no key, no misconception labels.
fn shown(record: &PlacementQuestionRecord) -> PlacementQuestion {
    PlacementQuestion {
        id: record.question.id,
        number: record.number,
        stem: record.question.stem.clone(),
        options: record
            .question
            .options
            .iter()
            .map(|option| option.text.clone())
            .collect(),
    }
}

/// An answer as the search sees it: the key is right, another option is
/// wrong with the misconception it represents.
fn search_answer(question: &KeyedPlacementQuestion, answer: PlacementAnswer) -> SearchAnswer {
    match answer {
        PlacementAnswer::NotSure => SearchAnswer::NotSure,
        PlacementAnswer::Option { index } if index == question.key => SearchAnswer::Right,
        PlacementAnswer::Option { index } => SearchAnswer::Wrong {
            misconception: question
                .options
                .get(index as usize)
                .and_then(|option| option.misconception.clone()),
        },
    }
}

/// A structured task's failure as the interface sees it. What was wrong with
/// an answer names the model's text, so it stays in the core.
fn task_error(error: StructuredError, what: &str) -> CoreError {
    match error {
        StructuredError::Provider(error) => error.into(),
        StructuredError::Invalid { .. } => CoreError::Provider {
            message: format!("the model's {what} did not hold, even after a repair"),
        },
    }
}

fn not_in_placement(course_id: Uuid) -> CoreError {
    CoreError::InvalidRequest {
        message: format!("course {course_id} is not in placement"),
    }
}
