pub const INSERT_ENCOUNTER: &str = 
r"
INSERT INTO encounter (
    last_combat_packet,
    total_damage_dealt,
    top_damage_dealt,
    total_damage_taken,
    top_damage_taken,
    dps,
    buffs,
    debuffs,
    total_shielding,
    total_effective_shielding,
    applied_shield_buffs,
    misc,
    version,
    boss_hp_log,
    stagger_log
)
VALUES
(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)";

pub const INSERT_ENTITIES: &str = r"
INSERT INTO entity (
    name,
    encounter_id,
    npc_id,
    entity_type,
    class_id,
    class,
    gear_score,
    current_hp,
    max_hp,
    is_dead,
    skills,
    damage_stats,
    skill_stats,
    dps,
    character_id,
    engravings,
    gear_hash,
    ark_passive_active,
    spec,
    ark_passive_data
)
VALUES
(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)";

pub const INSERT_ENCOUNTER_PREVIEW: &str = r"
INSERT INTO encounter_preview (
    id,
    fight_start,
    current_boss,
    duration,
    players,
    difficulty,
    local_player,
    my_dps,
    cleared,
    boss_only_damage
) 
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";