use rusqlite::{params, Connection};
use anyhow::*;

use crate::models::EntityDb;

use super::{queries::INSERT_ENTITIES, SqliteRepository};

impl SqliteRepository {

    pub(crate) fn insert_entities_inner<'a>(
        &self,
        connection: &Connection,
        encounter_id: i64,
        entities: &[EntityDb<'a>]) -> Result<()> {

        let mut statement = connection.prepare_cached(INSERT_ENTITIES)?;

        for entity in entities {

            let params = params![
                entity.name,
                encounter_id,
                entity.npc_id,
                entity.entity_type,
                entity.class_id,
                entity.class,
                entity.gear_score,
                entity.current_hp,
                entity.max_hp,
                entity.is_dead,
                entity.compressed_skills,
                entity.compressed_damage_stats,
                entity.skill_stats_json,
                entity.damage_stats.dps,
                entity.character_id,
                entity.engraving_data_json,
                entity.gear_hash,
                entity.ark_passive_active,
                entity.spec,
                entity.ark_passive_data_json
            ];

            statement.execute(params)?;
        }

        Ok(())
    }
}