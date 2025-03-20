use std::{cmp::{max, Reverse}, collections::BTreeMap};

use hashbrown::HashMap;
use lost_metrics_core::models::*;
use lost_metrics_misc::*;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, params_from_iter};
use anyhow::*;
use serde_json::{json, Value};

use crate::models::EntityDb;

pub const WINDOW_MS: i64 = 5_000;
pub const WINDOW_S: i64 = 5;

pub fn create_identity_logs_for_local_player(
    identity_log: &IdentityLog,
    class: &str,
    fight_start: i64) -> String {
  
    let mut total_identity_gain = 0;
    let data = identity_log;
    let duration_seconds = (data[data.len() - 1].0 - data[0].0) / 1000;
    let max = match class {
        "Summoner" => 7_000.0,
        "Souleater" => 3_000.0,
        _ => 10_000.0,
    };

    let stats: String = match class {
        "Arcanist" => {
            let mut cards: HashMap<u32, u32> = HashMap::new();
            let mut log: Vec<(i32, (f32, u32, u32))> = Vec::new();
            for i in 1..data.len() {
                let (t1, prev) = data[i - 1];
                let (t2, curr) = data[i];

                // don't count clown cards draws as card draws
                if curr.1 != 0 && curr.1 != prev.1 && prev.1 != 19284 {
                    cards.entry(curr.1).and_modify(|e| *e += 1).or_insert(1);
                }
                if curr.2 != 0 && curr.2 != prev.2 && prev.2 != 19284 {
                    cards.entry(curr.2).and_modify(|e| *e += 1).or_insert(1);
                }

                if t2 > t1 && curr.0 > prev.0 {
                    total_identity_gain += curr.0 - prev.0;
                }

                let relative_time = ((t2 - fight_start) as f32 / 1000.0) as i32;
                // calculate percentage, round to 2 decimal places
                let percentage = if curr.0 >= max as u32 {
                    100.0
                } else {
                    (((curr.0 as f32 / max) * 100.0) * 100.0).round() / 100.0
                };
                log.push((relative_time, (percentage, curr.1, curr.2)));
            }

            let avg_per_s = (total_identity_gain as f64 / duration_seconds as f64)
                / max as f64
                * 100.0;
            let identity_stats = IdentityArcanist {
                average: avg_per_s,
                card_draws: cards,
                log,
            };

            serde_json::to_string(&identity_stats).unwrap()
        }
        "Artist" | "Bard" => {
            let mut log: Vec<(i32, (f32, u32))> = Vec::new();

            for i in 1..data.len() {
                let (t1, i1) = data[i - 1];
                let (t2, i2) = data[i];

                if t2 <= t1 {
                    continue;
                }

                if i2.0 > i1.0 {
                    total_identity_gain += i2.0 - i1.0;
                }

                let relative_time = ((t2 - fight_start) as f32 / 1000.0) as i32;
                // since bard and artist have 3 bubbles, i.1 is the number of bubbles
                // we scale percentage to 3 bubbles
                // current bubble + max * number of bubbles
                let percentage: f32 =
                    ((((i2.0 as f32 + max * i2.1 as f32) / max) * 100.0) * 100.0)
                        .round()
                        / 100.0;
                log.push((relative_time, (percentage, i2.1)));
            }

            let avg_per_s = (total_identity_gain as f64 / duration_seconds as f64)
                / max as f64
                * 100.0;
            let identity_stats = IdentityArtistBard {
                average: avg_per_s,
                log,
            };
            serde_json::to_string(&identity_stats).unwrap()
        }
        _ => {
            let mut log: Vec<(i32, f32)> = Vec::new();
            for i in 1..data.len() {
                let (t1, i1) = data[i - 1];
                let (t2, i2) = data[i];

                if t2 <= t1 {
                    continue;
                }

                if i2.0 > i1.0 {
                    total_identity_gain += i2.0 - i1.0;
                }

                let relative_time = ((t2 - fight_start) as f32 / 1000.0) as i32;
                let percentage =
                    (((i2.0 as f32 / max) * 100.0) * 100.0).round() / 100.0;
                log.push((relative_time, percentage));
            }

            let avg_per_s = (total_identity_gain as f64 / duration_seconds as f64)
                / max as f64
                * 100.0;
            let identity_stats = IdentityGeneric {
                average: avg_per_s,
                log,
            };

            serde_json::to_string(&identity_stats).unwrap()
        }
    };

    stats
}

pub fn calculate_dps_rolling_10s_avg(intervals: &[i64], damage_log: &[(i64, i64)], fight_start: i64) -> Vec<i64> {
    let mut dps_rolling_10s_avg = vec![];

    for interval in intervals {
        let start = fight_start + interval - WINDOW_MS;
        let end = fight_start + interval + WINDOW_MS;

        let damage = sum_in_range(damage_log, start, end);
        dps_rolling_10s_avg.push(damage / (WINDOW_S * 2));
    }

    dps_rolling_10s_avg
}

pub fn create_stagger_stats(
    stagger_log: Vec<(i32, f32)>,
    encounter: &Encounter,
    prev_stagger: i32,
    stagger_intervals: &mut Vec<(i32, i32)>) -> Option<StaggerStats> {
    let mut stagger_stats: Option<StaggerStats> = None;

    if stagger_log.is_empty() {
        return None;
    }

    if prev_stagger > 0 && prev_stagger != encounter.encounter_damage_stats.max_stagger {
        // never finished staggering the boss, calculate average from whatever stagger has been done
        let stagger_start_s = ((encounter.encounter_damage_stats.stagger_start
            - encounter.fight_start)
            / 1000) as i32;
        let stagger_duration = stagger_log.last().unwrap().0 - stagger_start_s;
        if stagger_duration > 0 {
            stagger_intervals.push((stagger_duration, prev_stagger));
        }
    }

    let (total_stagger_time, total_stagger_dealt) = stagger_intervals.iter().fold(
        (0, 0),
        |(total_time, total_stagger), (time, stagger)| {
            (total_time + time, total_stagger + stagger)
        },
    );

    if total_stagger_time > 0 {
        let stagger = StaggerStats {
            average: (total_stagger_dealt as f64 / total_stagger_time as f64)
                / encounter.encounter_damage_stats.max_stagger as f64
                * 100.0,
            staggers_per_min: (total_stagger_dealt as f64 / (total_stagger_time as f64 / 60.0))
                / encounter.encounter_damage_stats.max_stagger as f64,
            log: stagger_log,
        };
        stagger_stats = Some(stagger);
    }

    stagger_stats
}

fn update_skill_cast_log(
    entity_id: u64,
    skills: &mut HashMap<u32, Skill>,
    skill_cast_log: &HashMap<u64, HashMap<u32, BTreeMap<i64, SkillCast>>>,) {
    for (_, skill_cast_log) in skill_cast_log.iter().filter(|&(s, _)| *s == entity_id) {
        for (skill, log) in skill_cast_log {
            skills.entry(*skill).and_modify(|e| {
                let average_cast = e.total_damage as f64 / e.casts as f64;
                let filter = average_cast * 0.05;
                let mut adj_hits = 0;
                let mut adj_crits = 0;
                for cast in log.values() {
                    for hit in cast.hits.iter() {
                        if hit.damage as f64 > filter {
                            adj_hits += 1;
                            if hit.crit {
                                adj_crits += 1;
                            }
                        }
                    }
                }

                if adj_hits > 0 {
                    e.adjusted_crit = Some(adj_crits as f64 / adj_hits as f64);
                }

                e.max_damage_cast = log
                    .values()
                    .map(|cast| cast.hits.iter().map(|hit| hit.damage).sum::<i64>())
                    .max()
                    .unwrap_or_default();
                e.skill_cast_log = log
                    .iter()
                    .map(|(_, skill_casts)| skill_casts.clone())
                    .collect();
            });
        }
    }
}

fn update_player_stats(
    entity: &mut EncounterEntity,
    player_info: Option<&HashMap<String, PlayerStats>>,
    damage_log: &HashMap<String, Vec<(i64, i64)>>,
    encounter_damage_stats: &EncounterDamageStats,
    fight_start: i64,
    last_combat_packet: i64,
    intervals: &[i64],
) {
    if let Some(damage_log) = damage_log.get(&entity.name) {
        if !&intervals.is_empty() {
            entity.damage_stats.dps_rolling_10s_avg = calculate_dps_rolling_10s_avg(intervals, &damage_log, fight_start);
        }

        let fight_start_sec = fight_start / 1000;
        let fight_end_sec = last_combat_packet / 1000;
        entity.damage_stats.dps_average =
            calculate_average_dps(damage_log, fight_start_sec, fight_end_sec);
    }

    let spec = get_player_spec(entity, &encounter_damage_stats.buffs);

    entity.spec = Some(spec.clone());
    let player_stats = player_info
        .and_then(|stats| stats.get(&entity.name));

    if let Some(info) = player_stats
    {
        for gem in info.gems.iter().flatten() {
            for skill_id in gem_skill_id_to_skill_ids(gem.skill_id) {
                if let Some(skill) = entity.skills.get_mut(&skill_id) {
                    match gem.gem_type {
                        5 | 34 => {
                            // damage gem
                            skill.gem_damage =
                                Some(damage_gem_value_to_level(gem.value, gem.tier));
                            skill.gem_tier_dmg = Some(gem.tier);
                        }
                        27 | 35 => {
                            // cooldown gem
                            skill.gem_cooldown =
                                Some(cooldown_gem_value_to_level(gem.value, gem.tier));
                            skill.gem_tier = Some(gem.tier);
                        }
                        64 | 65 => {
                            // support identity gem??
                            skill.gem_damage =
                                Some(support_damage_gem_value_to_level(gem.value));
                            skill.gem_tier_dmg = Some(gem.tier);
                        }
                        _ => {}
                    }
                }
            }
        }

        entity.ark_passive_active = Some(info.ark_passive_enabled);

        let (class, other) = get_engravings(entity.class_id, &info.engravings);
        entity.engraving_data = other;
        if info.ark_passive_enabled {
            if spec == "Unknown" {
                // not reliable enough to be used on its own
                if let Some(tree) = info.ark_passive_data.as_ref() {
                    if let Some(enlightenment) = tree.enlightenment.as_ref() {
                        for node in enlightenment.iter() {
                            let spec = get_spec_from_ark_passive(node);
                            if spec != "Unknown" {
                                entity.spec = Some(spec);
                                break;
                            }
                        }
                    }
                }
            }
            entity.ark_passive_data = info.ark_passive_data.clone();
        } else if class.len() == 1 {
            entity.spec = Some(class[0].clone());
        }
    }
}

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
                &encounter_damage_stats,
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

