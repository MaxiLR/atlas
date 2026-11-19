//! The settings of this installation: the model picked and the host id. They
//! are not syncable records and never leave the machine.

use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use super::{Store, StoreError};

const SELECTED_MODEL: &str = "selected_model";
const HOST_ID: &str = "host_id";

impl Store {
    /// The model the candidate picked, if any.
    pub fn selected_model(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .connection()
            .query_row(
                "SELECT value FROM local_settings WHERE key = ?1",
                [SELECTED_MODEL],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Keeps the candidate's pick, or clears it.
    pub fn set_selected_model(&self, slug: Option<&str>) -> Result<(), StoreError> {
        let connection = self.connection();
        match slug {
            Some(slug) => connection.execute(
                "INSERT INTO local_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![SELECTED_MODEL, slug],
            )?,
            None => connection.execute(
                "DELETE FROM local_settings WHERE key = ?1",
                [SELECTED_MODEL],
            )?,
        };
        Ok(())
    }

    /// This installation's host id, generated the first time it is asked for.
    pub fn host_id(&self) -> Result<Uuid, StoreError> {
        let connection = self.connection();
        connection.execute(
            "INSERT INTO local_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO NOTHING",
            params![HOST_ID, Uuid::new_v4().to_string()],
        )?;
        let id: super::Id = connection.query_row(
            "SELECT value FROM local_settings WHERE key = ?1",
            [HOST_ID],
            |row| row.get(0),
        )?;
        Ok(id.0)
    }
}
