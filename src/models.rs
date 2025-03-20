
use std::collections::BTreeMap;

use hashbrown::HashMap;
use lost_metrics_core::models::*;
use serde_json::Value;

pub struct CreateEncounter {
    pub encounter: Encounter,
    pub prev_stagger: i32,
    pub damage_log: HashMap<String, Vec<(i64, i64)>>,
    pub identity_log: HashMap<String, IdentityLog>,
    pub cast_log: HashMap<String, HashMap<u32, Vec<i32>>>,
    pub boss_hp_log: HashMap<String, Vec<BossHpLog>>,
    pub stagger_log: Vec<(i32, f32)>,
    pub stagger_intervals: Vec<(i32, i32)>,
    pub raid_clear: bool,
    pub party_info: Vec<Vec<String>>,
    pub raid_difficulty: String,
    pub region: Option<String>,
    pub player_info: Option<HashMap<String, PlayerStats>>,
    pub version: String,
    pub ntp_fight_start: i64,
    pub rdps_valid: bool,
    pub manual: bool,
    pub skill_cast_log: HashMap<u64, HashMap<u32, BTreeMap<i64, SkillCast>>>,
}

pub struct EncounterDb {
    pub last_combat_packet: i64,
    pub total_damage_dealt: i64,
    pub top_damage_dealt: i64,
    pub total_damage_taken: i64,
    pub top_damage_taken: i64,
    pub dps: i64,
    pub compressed_buffs: Vec<u8>,
    pub compressed_debuffs: Vec<u8>,
    pub total_shielding: u64,
    pub total_effective_shielding: u64,
    pub compressed_shields: Vec<u8>,
    pub misc_json: Value,
    pub db_version: i32,
    pub compressed_boss_hp: Vec<u8>,
    pub stagger_stats_json: Value
}

pub struct EncounterPreviewDb<'a> {
    pub fight_start: i64,
    pub current_boss_name: &'a str,
    pub duration: i64,
    pub preview_players: &'a str,
    pub raid_difficulty: &'a str,
    pub local_player: &'a str,
    pub local_player_dps: i64,
    pub raid_clear: Option<bool>,
    pub boss_only_damage: bool
}

pub struct EntityDb<'a> {
    pub id: u64,
    pub character_id: u64,
    pub npc_id: u32,
    pub name: &'a str,
    pub entity_type: String,
    pub class_id: u32,
    pub class: &'a str,
    pub gear_score: f32,
    pub current_hp: i64,
    pub max_hp: i64,
    pub current_shield: u64,
    pub is_dead: bool,
    pub skills: &'a HashMap<u32, Skill>,
    pub damage_stats: DamageStats,
    pub skill_stats: SkillStats,
    pub engraving_data: Option<Vec<String>>,
    pub gear_hash: Option<String>,
    pub ark_passive_active: Option<bool>,
    pub ark_passive_data: Option<ArkPassiveData>,
    pub spec: Option<&'a String>,
    pub compressed_damage_stats: Vec<u8>,
    pub compressed_skills: Vec<u8>,
    pub skill_stats_json: Value,
    pub engraving_data_json: Value,
    pub ark_passive_data_json: Value
}