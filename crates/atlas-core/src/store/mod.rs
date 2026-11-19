//! SQLite store with versioned, forward-only migrations, and the repository
//! functions every feature reads and writes through; no feature writes SQL of
//! its own. The database path and the clock are injected; nothing here knows
//! where the app keeps its data.
//!
//! Every syncable record has a UUID, a creation time, an update time and a
//! tombstone (ADR-0008). Deleting a record sets its tombstone; reads never
//! return tombstoned rows. Section versions are immutable, enforced by the
//! database. The settings of this installation are kept apart and never
//! synced.
//!
//! One file per area: `courses` (courses, ladder, placement, profile),
//! `index` (the index tree, the course map and the conventions), `sections`
//! (versions, records, check answers), `glossary`, `chats` and `settings`.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, SecondsFormat, SubsecRound, Utc};
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, Type, ValueRef};
use rusqlite::{Connection, Row, ToSql, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::domain::conventions::ConventionKind;
use crate::domain::profile::Knowledge;
use crate::domain::record::RecordStatus;
use crate::ports::Clock;
use crate::protocol::{
    ChatRole, CoreError, CourseStage, Goal, MessageState, SelfRating, VersionStyle,
};

/// Stores an enumeration as the snake_case text the schema's checks name.
macro_rules! sql_text {
    ($type:ty { $($variant:path => $text:literal),+ $(,)? }) => {
        impl rusqlite::ToSql for $type {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(rusqlite::types::ToSqlOutput::from(match self {
                    $($variant => $text),+
                }))
            }
        }

        impl rusqlite::types::FromSql for $type {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                match value.as_str()? {
                    $($text => Ok($variant),)+
                    other => Err(rusqlite::types::FromSqlError::Other(
                        format!("{other:?} is not a {}", stringify!($type)).into(),
                    )),
                }
            }
        }
    };
}

mod chats;
mod courses;
mod glossary;
mod index;
mod sections;
mod settings;

pub use chats::{NewChatMessage, SideChatRecord};
pub use courses::{CourseRecord, PlacementQuestionRecord};
pub use glossary::NewGlossaryEntry;
pub use index::{ConventionOutcome, NodeKind, NodeRecord};
pub use sections::{CheckAnswerRecord, CheckBlock, NewSectionVersion, StoredSectionVersion};

/// Migration `n` (1-based) is `MIGRATIONS[n - 1]`; `PRAGMA user_version`
/// records the last one applied. Migrations only ever move forward.
const MIGRATIONS: [&str; 3] = [
    include_str!("migrations/0001_thin_loop.sql"),
    include_str!("migrations/0002_full_schema.sql"),
    include_str!("migrations/0003_erase_tombstoned_versions.sql"),
];

/// The schema version this code migrates to.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(
        "the database is at schema version {found}, newer than the {supported} this app \
         reads: it was written by a newer Atlas"
    )]
    TooNew { found: i64, supported: i64 },
    #[error("{0} not found")]
    NotFound(String),
    /// A map entry names a concept the course does not have.
    #[error("the course has no concept {0:?}")]
    UnknownConcept(String),
    #[error("a stored value could not be encoded or decoded: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<StoreError> for CoreError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound(what) => CoreError::NotFound { what },
            error => CoreError::Store {
                message: error.to_string(),
            },
        }
    }
}

/// The kinds of syncable record, each one table. Any of them can be deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKind {
    Course,
    LadderRung,
    PlacementQuestion,
    ProfileRung,
    Misconception,
    IndexNode,
    SectionVersion,
    CheckAnswer,
    Concept,
    MapEntry,
    /// One concept an entry introduces or relies on.
    MapEntryConcept,
    Convention,
    GlossaryEntry,
    SideChat,
    ChatMessage,
}

impl RecordKind {
    fn table(self) -> &'static str {
        match self {
            Self::Course => "courses",
            Self::LadderRung => "ladder_rungs",
            Self::PlacementQuestion => "placement_questions",
            Self::ProfileRung => "profile_rungs",
            Self::Misconception => "misconceptions",
            Self::IndexNode => "index_nodes",
            Self::SectionVersion => "section_versions",
            Self::CheckAnswer => "check_answers",
            Self::Concept => "concepts",
            Self::MapEntry => "map_entries",
            Self::MapEntryConcept => "map_entry_concepts",
            Self::Convention => "conventions",
            Self::GlossaryEntry => "glossary_entries",
            Self::SideChat => "side_chats",
            Self::ChatMessage => "side_chat_messages",
        }
    }
}

pub struct Store {
    connection: Mutex<Connection>,
    clock: Arc<dyn Clock>,
}

impl Store {
    /// Opens (creating if missing) the database at `path` and migrates it.
    /// Every time the store writes comes from `clock`.
    pub fn open(path: &Path, clock: Arc<dyn Clock>) -> Result<Self, StoreError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        // Content a write replaces or frees is overwritten with zeros, so an
        // erased course cannot be read back from the file.
        connection.pragma_update(None, "secure_delete", true)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            clock,
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection.lock().expect("store lock poisoned")
    }

    /// The clock's time at the precision stored.
    fn now(&self) -> DateTime<Utc> {
        self.clock.now().trunc_subsecs(6)
    }

    /// Sets the tombstone of a live record, which keeps its id and creation
    /// time; the deletion is its last update. Returns whether a live record
    /// was found.
    pub fn delete(&self, kind: RecordKind, id: Uuid) -> Result<bool, StoreError> {
        let changed = self.connection().execute(
            &format!(
                "UPDATE {} SET deleted_at = ?1, updated_at = ?1
                 WHERE id = ?2 AND deleted_at IS NULL",
                kind.table()
            ),
            params![Time(self.now()), Id(id)],
        )?;
        Ok(changed > 0)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let applied: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if applied > SCHEMA_VERSION {
        return Err(StoreError::TooNew {
            found: applied,
            supported: SCHEMA_VERSION,
        });
    }
    for (version, sql) in (1_i64..)
        .zip(MIGRATIONS)
        .skip_while(|(version, _)| *version <= applied)
    {
        let transaction = connection.transaction()?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
        transaction.commit()?;
    }
    Ok(())
}

/// Fails with `NotFound(what)` unless a statement changed a row.
fn changed_one(changed: usize, what: impl FnOnce() -> String) -> Result<(), StoreError> {
    if changed == 0 {
        Err(StoreError::NotFound(what()))
    } else {
        Ok(())
    }
}

/// A UUID as stored: hyphenated text.
struct Id(Uuid);

impl ToSql for Id {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0.to_string()))
    }
}

impl FromSql for Id {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Uuid::parse_str(value.as_str()?)
            .map(Id)
            .map_err(|error| FromSqlError::Other(Box::new(error)))
    }
}

/// A time as stored: RFC 3339 in UTC, to the microsecond.
struct Time(DateTime<Utc>);

impl ToSql for Time {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(
            self.0.to_rfc3339_opts(SecondsFormat::Micros, true),
        ))
    }
}

impl FromSql for Time {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        DateTime::parse_from_rfc3339(value.as_str()?)
            .map(|time| Time(time.with_timezone(&Utc)))
            .map_err(|error| FromSqlError::Other(Box::new(error)))
    }
}

fn id_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    Ok(row.get::<_, Id>(index)?.0)
}

fn optional_id_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Uuid>> {
    Ok(row.get::<_, Option<Id>>(index)?.map(|id| id.0))
}

fn time_at(row: &Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    Ok(row.get::<_, Time>(index)?.0)
}

fn optional_time_at(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    Ok(row.get::<_, Option<Time>>(index)?.map(|time| time.0))
}

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, StoreError> {
    Ok(serde_json::to_string(value)?)
}

/// A JSON column decoded as `T`.
fn json_at<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let text: String = row.get(index)?;
    serde_json::from_str(&text)
        .map_err(|error| rusqlite::Error::FromSqlConversionFailure(index, Type::Text, error.into()))
}

fn optional_json_at<T: DeserializeOwned>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    match row.get::<_, Option<String>>(index)? {
        None => Ok(None),
        Some(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, error.into())
        }),
    }
}

sql_text!(CourseStage {
    CourseStage::Placement => "placement",
    CourseStage::Ready => "ready",
});

sql_text!(SelfRating {
    SelfRating::NewToThis => "new_to_this",
    SelfRating::KnowTheBasics => "know_the_basics",
    SelfRating::UseItRegularly => "use_it_regularly",
    SelfRating::Expert => "expert",
});

sql_text!(Goal {
    Goal::Overview => "overview",
    Goal::WorkingKnowledge => "working_knowledge",
    Goal::Expert => "expert",
});

sql_text!(Knowledge {
    Knowledge::Known => "known",
    Knowledge::Shaky => "shaky",
    Knowledge::Unknown => "unknown",
});

sql_text!(VersionStyle {
    VersionStyle::Original => "original",
    VersionStyle::Simpler => "simpler",
    VersionStyle::MoreTechnical => "more_technical",
    VersionStyle::MoreExamples => "more_examples",
});

sql_text!(RecordStatus {
    RecordStatus::Confirmed => "confirmed",
    RecordStatus::Predicted => "predicted",
});

sql_text!(ConventionKind {
    ConventionKind::Notation => "notation",
    ConventionKind::Terminology => "terminology",
    ConventionKind::RunningExample => "running_example",
});

sql_text!(ChatRole {
    ChatRole::Candidate => "candidate",
    ChatRole::Agent => "agent",
});

sql_text!(MessageState {
    MessageState::Complete => "complete",
    MessageState::Stopped => "stopped",
});
