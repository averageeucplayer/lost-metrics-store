use std::collections::BTreeMap;

use hashbrown::HashMap;
use lost_metrics_core::models::*;
use lost_metrics_misc::*;
use serde_json::json;

use crate::models::EntityDb;


pub fn to_entities_db<'a>(
    filtered: Vec<&'a mut EncounterEntity>,
    damage_log: HashMap<String, Vec<(i64, i64)>>,
    intervals: Vec<i64>,
    identity_log: HashMap<String, IdentityLog>,
    fight_start: i64,
    last_combat_packet: i64,
    encounter_damage_stats: &EncounterDamageStats,
    player_info: Option<&HashMap<String, PlayerStats>>,
    local_player: &str,
    duration_seconds: i64,
    skill_cast_log: HashMap<u64, HashMap<u32, BTreeMap<i64, SkillCast>>>,
    cast_log: HashMap<String, HashMap<u32, Vec<i32>>>
) -> Vec<EntityDb<'a>> {
    let mut entities = vec![];

    for entity in filtered {

        if entity.entity_type == EntityType::Player {
            update_player_stats(
                entity,
                player_info,
                &damage_log, 
                encounter_damage_stats,
                fight_start,
                last_combat_packet,
                &intervals);
        }

        let skills = &mut entity.skills;
        entity.damage_stats.dps = entity.damage_stats.damage_dealt / duration_seconds;

        for (_, skill) in skills.iter_mut() {
            skill.dps = skill.total_damage / duration_seconds;
        }

        let cast_log_entries = cast_log.iter().filter(|&(s, _)| *s == entity.name);
        for (_, cast_log) in cast_log_entries {
            for (skill, log) in cast_log {
                skills.entry(*skill).and_modify(|skill| {
                    skill.cast_log.clone_from(log);
                });
            }
        }

        update_skill_cast_log(entity.id, skills, &skill_cast_log);

        let mut stats = None;
        let identity_log_entries = identity_log.get(&entity.name)
            .filter(|identity_log| entity.name == local_player && identity_log.len() >= 2);

        if let Some(identity_log_entries) = identity_log_entries {
            stats = Some(create_identity_logs_for_local_player(identity_log_entries, &entity.class, fight_start));
        }

        if stats.is_some() {
            entity.skill_stats.identity_stats = stats;
        }

        let compressed_skills = compress_json(skills);
        let compressed_damage_stats = compress_json(&entity.damage_stats);

        let entity_db = EntityDb {
            id: entity.id,
            compressed_damage_stats,
            compressed_skills,
            character_id: entity.character_id,
            npc_id: entity.npc_id,
            name: &entity.name,
            entity_type: entity.entity_type.to_string(),
            class_id: entity.class_id,
            class: &entity.class,
            gear_score: entity.gear_score,
            current_hp: entity.current_hp,
            max_hp: entity.max_hp,
            current_shield: entity.current_shield,
            is_dead: entity.is_dead,
            skills: &entity.skills,
            damage_stats: entity.damage_stats.clone(),
            skill_stats: entity.skill_stats.clone(),
            engraving_data: entity.engraving_data.clone(),
            gear_hash: entity.gear_hash.clone(),
            ark_passive_active: entity.ark_passive_active,
            ark_passive_data: entity.ark_passive_data.clone(),
            spec: entity.spec.as_ref(),
            skill_stats_json: json!(entity.skill_stats),
            engraving_data_json: json!(entity.engraving_data),
            ark_passive_data_json: json!(entity.ark_passive_data)
        };

        entities.push(entity_db);
    }

    entities
}

