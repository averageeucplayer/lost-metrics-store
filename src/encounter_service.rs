use std::cmp::{max, Reverse};

use crate::{models::*, repository::Repository, utils::{create_stagger_stats, to_entities_db}};
use anyhow::*;
use lost_metrics_core::models::EncounterMisc;
use lost_metrics_misc::{compress_json, generate_intervals};
use serde_json::json;

pub const DB_VERSION: i32 = 5;

pub trait EncounterService : Send + Sync + 'static {
    fn create(&self, payload: CreateEncounter) -> Result<i64>;
}

pub struct DefaultEncounterService<R: Repository> {
    repository: R
}

impl<R: Repository> EncounterService for DefaultEncounterService<R> {
    fn create(&self, payload: CreateEncounter) -> Result<i64> {

        let mut encounter = payload.encounter;
        let raid_clear = payload.raid_clear;
        let party_info = payload.party_info;
        let rdps_valid = payload.rdps_valid;
        let version = payload.version;
        let manual = payload.manual;
        let region = payload.region;
        let prev_stagger = payload.prev_stagger;
        let ntp_fight_start = payload.ntp_fight_start;
        let stagger_log = payload.stagger_log;
        let raid_difficulty = payload.raid_difficulty;
        let boss_hp_log = payload.boss_hp_log;
        let skill_cast_log = payload.skill_cast_log;
        let damage_log = payload.damage_log;
        let cast_log = payload.cast_log;
        let mut stagger_intervals = payload.stagger_intervals;
        let player_info = payload.player_info;
        let identity_log = payload.identity_log;

        encounter.duration = encounter.last_combat_packet - encounter.fight_start;
        let duration_seconds = max(encounter.duration / 1000, 1);
        encounter.encounter_damage_stats.dps =
            encounter.encounter_damage_stats.total_damage_dealt / duration_seconds;

        let raid_clear = raid_clear.then_some(true);
        let party_info = (!party_info.is_empty()).then(|| 
            party_info
                .into_iter()
                .enumerate()
                .map(|(index, party)| (index as i32, party))
                .collect(),
        );
        let misc: EncounterMisc = EncounterMisc {
            raid_clear,
            party_info,
            region,
            version: Some(version),
            rdps_valid: Some(rdps_valid),
            rdps_message: if rdps_valid {
                None
            } else {
                Some("invalid_stats".to_string())
            },
            ntp_fight_start: Some(ntp_fight_start),
            manual_save: Some(manual),
            ..Default::default()
        };
        let local_player = &encounter.local_player;
        let encounter_damage_stats = &encounter.encounter_damage_stats;
        let fight_start = encounter.fight_start;
        let fight_end = encounter.last_combat_packet;
        let intervals = generate_intervals(fight_start, fight_end);
        let stagger_stats = create_stagger_stats(stagger_log, &encounter, prev_stagger, &mut stagger_intervals);
        let compressed_boss_hp = compress_json(&boss_hp_log);
        let compressed_buffs = compress_json(&encounter_damage_stats.buffs);
        let compressed_debuffs = compress_json(&encounter_damage_stats.debuffs);
        let compressed_shields = compress_json(&encounter_damage_stats.applied_shield_buffs);
        
        let mut players = encounter
            .entities
            .values()
            .filter(|entity| entity.is_active_player(local_player))
            .collect::<Vec<_>>();
        let local_player_dps = players
            .iter()
            .find(|e| &e.name == local_player)
            .map(|e| e.damage_stats.dps)
            .unwrap_or_default();
        players.sort_unstable_by_key(|e| Reverse(e.damage_stats.damage_dealt));
        let preview_players = players
            .into_iter()
            .map(|e| format!("{}:{}", e.class_id, e.name))
            .collect::<Vec<_>>()
            .join(",");

        let filtered: Vec<_> = encounter.entities
            .iter_mut()
            .filter(|(_, entity)| entity.is_relevant_combat_entity(local_player))
            .map(|(_, entity)| entity)
            .collect();

        let encounter_db = EncounterDb {
            last_combat_packet: encounter.last_combat_packet,
            total_damage_dealt: encounter_damage_stats.total_damage_dealt,
            top_damage_dealt: encounter_damage_stats.top_damage_dealt,
            total_damage_taken: encounter_damage_stats.total_damage_taken,
            top_damage_taken: encounter_damage_stats.top_damage_taken,
            dps: encounter_damage_stats.dps,
            compressed_buffs,
            compressed_debuffs,
            total_shielding: encounter_damage_stats.total_shielding,
            total_effective_shielding: encounter_damage_stats.total_effective_shielding,
            compressed_shields: compressed_shields,
            misc_json: json!(misc),
            db_version: DB_VERSION,
            compressed_boss_hp,
            stagger_stats_json: json!(stagger_stats),
        };
        
       
        let entities = to_entities_db(
            filtered,
            damage_log,
            intervals,
            identity_log,
            fight_start,
            encounter.last_combat_packet,
            encounter_damage_stats,
            player_info.as_ref(),
            &local_player,
            duration_seconds,
            skill_cast_log,
            cast_log);

        let encounter_preview = EncounterPreviewDb {
            fight_start: encounter.fight_start,
            current_boss_name: &encounter.current_boss_name,
            duration: encounter.duration,
            preview_players: &preview_players,
            raid_difficulty: &raid_difficulty,
            local_player: &local_player,
            local_player_dps,
            raid_clear,
            boss_only_damage: encounter.boss_only_damage
        };

        let encounter_id = self.insert_encounter_and_entities(encounter_db, encounter_preview, entities)?;

        Ok(encounter_id)
    }
}

impl<R: Repository> DefaultEncounterService<R> {
    pub fn new(repository: R) -> Self {
        Self {
            repository
        }
    }

    fn insert_encounter_and_entities<'a>(&self,
        encounter: EncounterDb,
        encounter_preview: EncounterPreviewDb<'a>,
        entities: Vec<EntityDb<'a>>) -> Result<i64>
    {
        let repository = &self.repository;
        let mut connection = self.repository.get_connection()?;
        let transaction = connection.transaction()?;

        let encounter_id = repository.insert_encounter(&transaction, encounter)?;
        repository.insert_entities(&transaction, encounter_id, &entities)?;
        repository.insert_encounter_preview(&transaction, encounter_id, encounter_preview)?;

        transaction.commit()?;

        Ok(encounter_id)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use hashbrown::{HashMap, HashSet};
    use lost_metrics_core::models::{DamageStats, Encounter, EncounterDamageStats, EncounterEntity, EntityType, MostDamageTakenEntity, SkillStats};

    use crate::{connection_pool, migration_runner::{self, MigrationRunner}, repository::SqliteRepository};

    use super::*;

    #[test]
    fn should_create_new_encounter() {
        let pool = connection_pool::in_memory();
        let repository = SqliteRepository::new(pool.clone());
        let migration_runner = MigrationRunner::new(pool);
        let service = DefaultEncounterService::new(repository);

        migration_runner.run().unwrap();

        let fight_start = Utc::now();
        let last_combat_packet = (fight_start + Duration::minutes(10)).timestamp_millis();
        let fight_start = fight_start.timestamp_millis();
        let duration = last_combat_packet - fight_start;

        let entities: HashMap<String, EncounterEntity> = HashMap::new();

        let mut party_info: HashMap<i32, Vec<String>> = HashMap::new();
        party_info.insert(0, vec!["player_1".into(), "player_2".into(), "player_3".into(), "player_4".into()]);
        party_info.insert(1, vec!["player_5".into(), "player_6".into(), "player_7".into(), "player_8".into()]);

        let party_info_vec= vec![
            vec!["player_1".into(), "player_2".into(), "player_3".into(), "player_4".into()],
            vec!["player_5".into(), "player_6".into(), "player_7".into(), "player_8".into()]
        ];

        let boss = EncounterEntity {
            id: 1,
            character_id: 0,
            npc_id: 1,
            name: "Narok the Butcher".into(),
            entity_type: EntityType::Boss,
            class_id: 0,
            class: "Unknown".into(),
            gear_score: 0.0,
            current_hp: 0,
            max_hp: 1000000,
            current_shield: 0,
            is_dead: true,
            skills: HashMap::new(),
            damage_stats: DamageStats::default(),
            skill_stats: SkillStats::default(),
            engraving_data: None,
            gear_hash: None,
            ark_passive_active: None,
            ark_passive_data: None,
            spec: Some("Unknown".into()),
        };

        let encounter_misc = EncounterMisc {
            stagger_stats: None,
            boss_hp_log: Some(HashMap::new()),
            raid_clear: Some(true),
            party_info: Some(party_info),
            region: Some("EUC".into()),
            version: Some("0.0.1".into()),
            rdps_valid: None,
            rdps_message: None,
            ntp_fight_start: None,
            manual_save: Some(false),
        };

        let encounter_damage_stats = EncounterDamageStats {
            total_damage_dealt: 100,
            top_damage_dealt: 10,
            total_damage_taken: 100,
            top_damage_taken: 10,
            dps: 2,
            most_damage_taken_entity: MostDamageTakenEntity {
                name: "test".into(),
                damage_taken: 10,
            },
            buffs: HashMap::new(),
            debuffs: HashMap::new(),
            total_shielding: 0,
            total_effective_shielding: 0,
            applied_shield_buffs: HashMap::new(),
            unknown_buffs: HashSet::new(),
            max_stagger: 0,
            stagger_start: 0,
            misc: Some(encounter_misc),
            boss_hp_log: HashMap::new(),
            stagger_stats: None,
        };

        let encounter = Encounter {
            last_combat_packet,
            fight_start,
            local_player: "test".into(),
            entities: entities,
            current_boss_name: "Narok the Butcher".into(),
            current_boss: Some(boss),
            encounter_damage_stats,
            duration,
            difficulty: Some("Hard".into()),
            favorite: true,
            cleared: true,
            boss_only_damage: true,
            sync: None,
        };

        let payload = CreateEncounter {
            encounter,
            prev_stagger: 0,
            damage_log: HashMap::new(),
            identity_log: HashMap::new(),
            cast_log: HashMap::new(),
            boss_hp_log: HashMap::new(),
            stagger_log: vec![],
            stagger_intervals: vec![],
            raid_clear: true,
            party_info: party_info_vec,
            raid_difficulty: "Hard".into(),
            region: Some("EUC".into()),
            player_info: None,
            version: "0.0.1".into(),
            ntp_fight_start: 0,
            rdps_valid: false,
            manual: false,
            skill_cast_log: HashMap::new(),
        };

        service.create(payload).unwrap();
    }
}