use anyhow::*;
use rusqlite::{params, Connection};

use crate::models::EncounterPreviewDb;

use super::{queries::INSERT_ENCOUNTER_PREVIEW, SqliteRepository};

impl SqliteRepository {

    pub(crate) fn insert_encounter_preview_inner<'a>(
        &self,
        connection: &Connection,
        encounter_id: i64,
        encounter_preview: EncounterPreviewDb<'a>) -> Result<()> {
        
        let mut statement = connection.prepare_cached(INSERT_ENCOUNTER_PREVIEW)?;

        let params = params![
            encounter_id,
            encounter_preview.fight_start,
            encounter_preview.current_boss_name,
            encounter_preview.duration,
            encounter_preview.preview_players,
            encounter_preview.raid_difficulty,
            encounter_preview.local_player,
            encounter_preview.local_player_dps,
            encounter_preview.raid_clear,
            encounter_preview.boss_only_damage
        ];

        statement.execute(params)?;
        
        Ok(())
    }
}