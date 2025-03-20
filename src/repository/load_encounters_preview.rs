use lost_metrics_core::models::*;
use rusqlite::{params_from_iter, Row};
use anyhow::*;

use super::SqliteRepository;

impl SqliteRepository {

    pub(crate) fn load_encounters_preview_inner(
        &self,
        page: i32,
        page_size: i32,
        search: String,
        filter: SearchFilter,
    ) -> Result<EncountersOverview> {
        let connection = self.pool.get()?;

        let mut params = vec![];

        let join_clause = if search.len() > 2 {
            let escaped_search = search
                .split_whitespace()
                .map(|word| format!("\"{}\"", word.replace("\"", "")))
                .collect::<Vec<_>>()
                .join(" ");
            params.push(escaped_search);
            "JOIN encounter_search(?) ON encounter_search.rowid = e.id"
        } else {
            ""
        };

        params.push((filter.min_duration * 1000).to_string());

        let boss_filter = if !filter.bosses.is_empty() {
            let mut placeholders = "?,".repeat(filter.bosses.len());
            placeholders.pop(); // remove trailing comma
            params.extend(filter.bosses);
            format!("AND e.current_boss IN ({})", placeholders)
        } else {
            "".to_string()
        };

        let raid_clear_filter = if filter.cleared {
            "AND cleared = 1"
        } else {
            ""
        };

        let favorite_filter = if filter.favorite {
            "AND favorite = 1"
        } else {
            ""
        };

        let boss_only_damage_filter = if filter.boss_only_damage {
            "AND boss_only_damage = 1"
        } else {
            ""
        };

        let difficulty_filter = if !filter.difficulty.is_empty() {
            params.push(filter.difficulty);
            "AND difficulty = ?"
        } else {
            ""
        };

        let order = if filter.order == 1 { "ASC" } else { "DESC" };
        let sort = format!("e.{}", filter.sort);

        let count_params = params.clone();

        let query = format!(
            "SELECT
        e.id,
        e.fight_start,
        e.current_boss,
        e.duration,
        e.difficulty,
        e.favorite,
        e.cleared,
        e.local_player,
        e.my_dps,
        e.players
        FROM encounter_preview e {}
        WHERE e.duration > ? {}
        {} {} {} {}
        ORDER BY {} {}
        LIMIT ?
        OFFSET ?",
            join_clause,
            boss_filter,
            raid_clear_filter,
            favorite_filter,
            difficulty_filter,
            boss_only_damage_filter,
            sort,
            order
        );

        let mut statement = connection.prepare_cached(&query).unwrap();

        let offset = (page - 1) * page_size;

        params.push(page_size.to_string());
        params.push(offset.to_string());

        let encounter_iter = statement
            .query_map(params_from_iter(params), |row| Self::map_to_row(row))
            .expect("could not query encounters");

        let encounters: Vec<EncounterPreview> = encounter_iter.collect::<Result<_, _>>().unwrap();

        let query = format!(
            "
            SELECT COUNT(*)
            FROM encounter_preview e {}
            WHERE duration > ? {}
            {} {} {} {}
            ",
            join_clause,
            boss_filter,
            raid_clear_filter,
            favorite_filter,
            difficulty_filter,
            boss_only_damage_filter
        );

        let count: i32 = connection
            .query_row_and_then(&query, params_from_iter(count_params), |row| row.get(0))
            .expect("could not get encounter count");

        let result = EncountersOverview {
            encounters,
            total_encounters: count,
        };

        Ok(result)
    }

    fn map_to_row(row: &Row) -> rusqlite::Result<EncounterPreview> {
        let classes: String = row.get(9).unwrap_or_default();

        let (classes, names) = classes
            .split(',')
            .map(|s| {
                let info: Vec<&str> = s.split(':').collect();
                if info.len() != 2 {
                    return (101, "Unknown".to_string());
                }
                (info[0].parse::<i32>().unwrap_or(101), info[1].to_string())
            })
            .unzip();

        std::result::Result::Ok(EncounterPreview {
            id: row.get(0)?,
            fight_start: row.get(1)?,
            boss_name: row.get(2)?,
            duration: row.get(3)?,
            classes,
            names,
            difficulty: row.get(4)?,
            favorite: row.get(5)?,
            cleared: row.get(6)?,
            local_player: row.get(7)?,
            my_dps: row.get(8).unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use hashbrown::{HashMap, HashSet};
    use lost_metrics_core::models::{DamageStats, Encounter, EncounterDamageStats, EncounterEntity, EntityType, MostDamageTakenEntity, SkillStats};
    use serde_json::json;

    use crate::{connection_pool, migration_runner::{self, MigrationRunner}, models::{EncounterDb, EncounterPreviewDb}, repository::{Repository, SqliteRepository}};

    use super::*;

    #[test]
    fn should_return_encounter() {
        let pool = connection_pool::in_memory();
        let repository = SqliteRepository::new(pool.clone());
        let migration_runner = MigrationRunner::new(pool);
        
        migration_runner.run().unwrap();

        let fight_start = Utc::now();
        let last_combat_packet = (fight_start + Duration::minutes(10)).timestamp_millis();

        let encounter = EncounterDb {
            last_combat_packet,
            total_damage_dealt: 0,
            top_damage_dealt: 0,
            total_damage_taken: 0,
            top_damage_taken: 0,
            dps: 0,
            compressed_buffs: vec![],
            compressed_debuffs: vec![],
            total_shielding: 0,
            total_effective_shielding: 0,
            compressed_shields: vec![],
            misc_json: json!(""),
            db_version: 5,
            compressed_boss_hp:  vec![],
            stagger_stats_json: json!(""),
        };

        let encounter_preview = EncounterPreviewDb {
            fight_start: 10000,
            current_boss_name: "Narok the Butcher".into(),
            duration: 100,
            preview_players: "test",
            raid_difficulty: "Hard".into(),
            local_player: "test",
            local_player_dps: 10,
            raid_clear: Some(true),
            boss_only_damage: true,
        };
        let connection = repository.get_connection().unwrap();
        let encounter_id = repository.insert_encounter_inner(&connection, encounter).unwrap();
        repository.insert_encounter_preview_inner(&connection, encounter_id, encounter_preview).unwrap();

        let filter = SearchFilter {
            sort: "fight_start".into(),
            ..Default::default()
        };

        let result = repository.load_encounters_preview_inner(0, 10, "".into(), filter).unwrap();
        assert_eq!(result.encounters.len(), 1);
    }
}