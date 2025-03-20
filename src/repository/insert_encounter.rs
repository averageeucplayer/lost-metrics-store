use rusqlite::{params, Connection};
use anyhow::*;

use crate::models::EncounterDb;

use super::{queries::INSERT_ENCOUNTER, SqliteRepository};

impl SqliteRepository {

    pub(crate) fn insert_encounter_inner(
        &self,
        connection: &Connection,
        encounter: EncounterDb) -> Result<i64> {

        let mut statement = connection.prepare_cached(INSERT_ENCOUNTER)?;

        let params = params![
            encounter.last_combat_packet,
            encounter.total_damage_dealt,
            encounter.top_damage_dealt,
            encounter.total_damage_taken,
            encounter.top_damage_taken,
            encounter.dps,
            encounter.compressed_buffs,
            encounter.compressed_debuffs,
            encounter.total_shielding,
            encounter.total_effective_shielding,
            encounter.compressed_shields,
            encounter.misc_json,
            encounter.db_version,
            encounter.compressed_boss_hp,
            encounter.stagger_stats_json,
        ];

        statement.execute(params)?;

        let encounter_id = connection.last_insert_rowid();
        
        Ok(encounter_id)
    }
}