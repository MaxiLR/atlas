//! Courses, their ladder, the placement questions asked and the candidate
//! profile.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use super::{
    Id, Store, StoreError, Time, changed_one, id_at, json, json_at, optional_id_at,
    optional_json_at, optional_time_at, time_at,
};
use crate::domain::ladder::{Ladder, Rung, RungId};
use crate::domain::profile::{CandidateProfile, Knowledge, Misconception, RungKnowledge};
use crate::domain::questions::KeyedPlacementQuestion;
use crate::protocol::{CourseStage, Goal, PlacementAnswer, SelfRating};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRecord {
    pub id: Uuid,
    pub topic: String,
    pub stage: CourseStage,
    pub self_rating: Option<SelfRating>,
    pub goal: Option<Goal>,
    pub last_opened_at: Option<DateTime<Utc>>,
    /// The section last read.
    pub last_read: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A placement question as asked, with its answer once given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementQuestionRecord {
    /// Its id is the record's.
    pub question: KeyedPlacementQuestion,
    /// 1 for the first question asked.
    pub number: u32,
    pub answer: Option<PlacementAnswer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COURSE_COLUMNS: &str = "id, topic, stage, self_rating, goal, last_opened_at, \
                              last_read_node_id, created_at, updated_at";

fn course_row(row: &Row<'_>) -> rusqlite::Result<CourseRecord> {
    Ok(CourseRecord {
        id: id_at(row, 0)?,
        topic: row.get(1)?,
        stage: row.get(2)?,
        self_rating: row.get(3)?,
        goal: row.get(4)?,
        last_opened_at: optional_time_at(row, 5)?,
        last_read: optional_id_at(row, 6)?,
        created_at: time_at(row, 7)?,
        updated_at: time_at(row, 8)?,
    })
}

fn placement_row(row: &Row<'_>) -> rusqlite::Result<PlacementQuestionRecord> {
    Ok(PlacementQuestionRecord {
        question: json_at(row, 0)?,
        number: row.get(1)?,
        answer: optional_json_at(row, 2)?,
        created_at: time_at(row, 3)?,
        updated_at: time_at(row, 4)?,
    })
}

/// Inserts a course row.
pub(super) fn insert_course(
    connection: &Connection,
    now: DateTime<Utc>,
    id: Uuid,
    topic: &str,
    stage: CourseStage,
) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO courses (id, topic, stage, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![Id(id), topic, stage, Time(now)],
    )?;
    Ok(())
}

/// Fails with `NotFound` unless the course is live.
pub(super) fn require_course(connection: &Connection, course: Uuid) -> Result<(), StoreError> {
    connection
        .query_row(
            "SELECT 1 FROM courses WHERE id = ?1 AND deleted_at IS NULL",
            [Id(course)],
            |_| Ok(()),
        )
        .optional()?
        .ok_or_else(|| StoreError::NotFound(format!("course {course}")))
}

impl Store {
    /// Stores a new course on `topic`, in the placement stage.
    pub fn create_course(&self, topic: &str) -> Result<CourseRecord, StoreError> {
        let now = self.now();
        let id = Uuid::new_v4();
        insert_course(&self.connection(), now, id, topic, CourseStage::Placement)?;
        Ok(CourseRecord {
            id,
            topic: topic.to_owned(),
            stage: CourseStage::Placement,
            self_rating: None,
            goal: None,
            last_opened_at: None,
            last_read: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn course_record(&self, course: Uuid) -> Result<Option<CourseRecord>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                &format!(
                    "SELECT {COURSE_COLUMNS} FROM courses WHERE id = ?1 AND deleted_at IS NULL"
                ),
                [Id(course)],
                course_row,
            )
            .optional()?)
    }

    /// Every course, most recently opened first; a course never opened counts
    /// from its creation.
    pub fn course_records(&self) -> Result<Vec<CourseRecord>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {COURSE_COLUMNS} FROM courses WHERE deleted_at IS NULL
             ORDER BY COALESCE(last_opened_at, created_at) DESC, created_at DESC, rowid DESC"
        ))?;
        let courses = statement
            .query_map([], course_row)?
            .collect::<Result<_, _>>()?;
        Ok(courses)
    }

    pub fn set_self_rating(
        &self,
        course: Uuid,
        rating: SelfRating,
        goal: Goal,
    ) -> Result<(), StoreError> {
        self.update_course(
            "self_rating = ?3, goal = ?4",
            course,
            params![Id(course), Time(self.now()), rating, goal],
        )
    }

    pub fn set_stage(&self, course: Uuid, stage: CourseStage) -> Result<(), StoreError> {
        self.update_course(
            "stage = ?3",
            course,
            params![Id(course), Time(self.now()), stage],
        )
    }

    /// Records that the course was opened now.
    pub fn mark_opened(&self, course: Uuid) -> Result<(), StoreError> {
        self.update_course(
            "last_opened_at = ?2",
            course,
            params![Id(course), Time(self.now())],
        )
    }

    /// Records the section the course resumes at.
    pub fn set_last_read(&self, course: Uuid, node: Uuid) -> Result<(), StoreError> {
        self.update_course(
            "last_read_node_id = ?3",
            course,
            params![Id(course), Time(self.now()), Id(node)],
        )
    }

    /// Sets `assignments` on a live course and moves its update time; `?1` is
    /// the course and `?2` the time.
    fn update_course(
        &self,
        assignments: &str,
        course: Uuid,
        parameters: &[&dyn rusqlite::ToSql],
    ) -> Result<(), StoreError> {
        let changed = self.connection().execute(
            &format!(
                "UPDATE courses SET {assignments}, updated_at = ?2
                 WHERE id = ?1 AND deleted_at IS NULL"
            ),
            parameters,
        )?;
        changed_one(changed, || format!("course {course}"))
    }

    /// Stores the course's ladder, rung by rung, in one transaction.
    pub fn store_ladder(&self, course: Uuid, ladder: &Ladder) -> Result<(), StoreError> {
        let now = Time(self.now());
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        require_course(&transaction, course)?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO ladder_rungs (id, course_id, rung, concept, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            )?;
            for rung in &ladder.rungs {
                insert.execute(params![
                    Id(Uuid::new_v4()),
                    Id(course),
                    rung.id.0,
                    rung.concept,
                    now
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// The course's ladder, lowest rung first, once drafted.
    pub fn ladder(&self, course: Uuid) -> Result<Option<Ladder>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT rung, concept FROM ladder_rungs
             WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY rung",
        )?;
        let rungs: Vec<Rung> = statement
            .query_map([Id(course)], |row| {
                Ok(Rung {
                    id: RungId(row.get(0)?),
                    concept: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok((!rungs.is_empty()).then_some(Ladder { rungs }))
    }

    /// Stores the profile in one transaction: the goal on the course, each
    /// rung's state (a rung already stored in that state is left as it is),
    /// and the misconceptions not stored yet.
    pub fn save_profile(&self, course: Uuid, profile: &CandidateProfile) -> Result<(), StoreError> {
        let now = Time(self.now());
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        require_course(&transaction, course)?;
        transaction.execute(
            "UPDATE courses SET goal = ?2, updated_at = ?3
             WHERE id = ?1 AND goal IS NOT ?2",
            params![Id(course), profile.goal, now],
        )?;
        for rung in &profile.rungs {
            let stored: Option<Knowledge> = transaction
                .query_row(
                    "SELECT state FROM profile_rungs
                     WHERE course_id = ?1 AND rung = ?2 AND deleted_at IS NULL",
                    params![Id(course), rung.rung.0],
                    |row| row.get(0),
                )
                .optional()?;
            match stored {
                Some(state) if state == rung.state => {}
                Some(_) => {
                    transaction.execute(
                        "UPDATE profile_rungs SET state = ?3, updated_at = ?4
                         WHERE course_id = ?1 AND rung = ?2 AND deleted_at IS NULL",
                        params![Id(course), rung.rung.0, rung.state, now],
                    )?;
                }
                None => {
                    transaction.execute(
                        "INSERT INTO profile_rungs
                            (id, course_id, rung, state, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                        params![Id(Uuid::new_v4()), Id(course), rung.rung.0, rung.state, now],
                    )?;
                }
            }
        }
        for misconception in &profile.misconceptions {
            transaction.execute(
                "INSERT INTO misconceptions (id, course_id, rung, text, created_at, updated_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?5
                 WHERE NOT EXISTS (SELECT 1 FROM misconceptions
                                   WHERE course_id = ?2 AND rung = ?3 AND text = ?4
                                     AND deleted_at IS NULL)",
                params![
                    Id(Uuid::new_v4()),
                    Id(course),
                    misconception.rung.0,
                    misconception.text,
                    now
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// The candidate profile, once placement has stored one: rungs in ladder
    /// order, misconceptions in the order recorded.
    pub fn profile(&self, course: Uuid) -> Result<Option<CandidateProfile>, StoreError> {
        let connection = self.connection();
        let goal: Option<Goal> = connection
            .query_row(
                "SELECT goal FROM courses WHERE id = ?1 AND deleted_at IS NULL",
                [Id(course)],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(goal) = goal else {
            return Ok(None);
        };
        let rungs: Vec<RungKnowledge> = connection
            .prepare(
                "SELECT rung, state FROM profile_rungs
                 WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY rung",
            )?
            .query_map([Id(course)], |row| {
                Ok(RungKnowledge {
                    rung: RungId(row.get(0)?),
                    state: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        if rungs.is_empty() {
            return Ok(None);
        }
        let misconceptions = connection
            .prepare(
                "SELECT rung, text FROM misconceptions
                 WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY created_at, rowid",
            )?
            .query_map([Id(course)], |row| {
                Ok(Misconception {
                    rung: RungId(row.get(0)?),
                    text: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Some(CandidateProfile {
            goal,
            rungs,
            misconceptions,
        }))
    }

    /// Stores a question as asked, numbered after the ones before it. The
    /// question's id is the record's.
    pub fn add_placement_question(
        &self,
        course: Uuid,
        question: &KeyedPlacementQuestion,
    ) -> Result<PlacementQuestionRecord, StoreError> {
        let now = self.now();
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        require_course(&transaction, course)?;
        let number: u32 = transaction.query_row(
            "SELECT COALESCE(MAX(number), 0) + 1 FROM placement_questions
             WHERE course_id = ?1 AND deleted_at IS NULL",
            [Id(course)],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO placement_questions
                (id, course_id, number, question, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                Id(question.id),
                Id(course),
                number,
                json(question)?,
                Time(now)
            ],
        )?;
        transaction.commit()?;
        Ok(PlacementQuestionRecord {
            question: question.clone(),
            number,
            answer: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn answer_placement_question(
        &self,
        question: Uuid,
        answer: PlacementAnswer,
    ) -> Result<(), StoreError> {
        let changed = self.connection().execute(
            "UPDATE placement_questions SET answer = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![Id(question), json(&answer)?, Time(self.now())],
        )?;
        changed_one(changed, || format!("placement question {question}"))
    }

    /// The questions asked, in the order asked.
    pub fn placement_questions(
        &self,
        course: Uuid,
    ) -> Result<Vec<PlacementQuestionRecord>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT question, number, answer, created_at, updated_at FROM placement_questions
             WHERE course_id = ?1 AND deleted_at IS NULL ORDER BY number",
        )?;
        let questions = statement
            .query_map([Id(course)], placement_row)?
            .collect::<Result<_, _>>()?;
        Ok(questions)
    }
}
