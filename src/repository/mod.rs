mod load_encounters_preview;
mod insert_encounter;
mod insert_entities;
mod insert_encounter_preview;
mod queries;

use lost_metrics_core::models::*;
use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use anyhow::*;

#[cfg(test)]
use mockall::automock;

use crate::models::*;

#[cfg_attr(test, automock)]
pub trait Repository : Send + Sync + 'static {
    fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>>;
    fn load_encounters_preview(
        &self,
        page: i32,
        page_size: i32,
        search: String,
        filter: SearchFilter,
    ) -> Result<EncountersOverview>;
    fn insert_encounter(
        &self,
        connection: &Connection,
        encounter: EncounterDb) -> Result<i64>;
    fn insert_entities<'a>(
        &self,
        connection: &Connection,
        encounter_id: i64,
        entities: &[EntityDb<'a>]) -> Result<()>;
    fn insert_encounter_preview<'a>(
        &self,
        connection: &Connection,
        encounter_id: i64,
        encounter_preview: EncounterPreviewDb<'a>) -> Result<()>;
}

pub struct SqliteRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl Repository for SqliteRepository {

    fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        let connection = self.pool.get()?;
        Ok(connection)
    }

    fn load_encounters_preview(
        &self,
        page: i32,
        page_size: i32,
        search: String,
        filter: SearchFilter) -> Result<EncountersOverview> {
        self.load_encounters_preview_inner(page, page_size, search, filter)
    }
    
    fn insert_entities<'a>(&self,
        connection: &Connection,
        encounter_id: i64,
        entities: &[EntityDb<'a>]) -> Result<()> {
        self.insert_entities_inner(connection, encounter_id, entities)
    }
 
    fn insert_encounter(
        &self,
        connection: &Connection,
        encounter: EncounterDb) -> Result<i64> {
        self.insert_encounter_inner(connection, encounter)
    }
    
    fn insert_encounter_preview<'a>(
        &self,
        connection: &Connection,
        encounter_id: i64,
        encounter_preview: EncounterPreviewDb<'a>) -> Result<()> {
        self.insert_encounter_preview_inner(connection, encounter_id, encounter_preview)
    }
}

impl SqliteRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {

        Self {
            pool,
        }
    }
}