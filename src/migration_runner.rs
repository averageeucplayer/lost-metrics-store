use log::*;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Transaction;
use anyhow::*;

pub struct MigrationRunner {
    pool: Pool<SqliteConnectionManager> 
}

impl MigrationRunner {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    pub fn run(&self) -> Result<()> {
        info!("setting up database");
        let mut connection= self.pool.get()?;
        let transaction = connection.transaction()?;
    
        // FIXME: replace me with idempotent migrations
    
        let mut statement = transaction.prepare("SELECT 1 FROM sqlite_master WHERE type=? AND name=?")?;
        if !statement.exists(["table", "encounter"])? {
            info!("creating tables");
            migration_legacy_encounter(&transaction)?;
            migration_legacy_entity(&transaction)?;
        }
    
        // NOTE: for databases, where the bad migration code already ran
        migration_legacy_entity(&transaction)?;
    
        if !statement.exists(["table", "encounter_preview"])? {
            info!("optimizing searches");
            migration_legacy_encounter(&transaction)?;
            migration_legacy_entity(&transaction)?;
            migration_full_text_search(&transaction)?;
        }
    
        if !statement.exists(["table", "sync_logs"])? {
            info!("adding sync table");
            migration_sync(&transaction)?;
        }
    
        migration_specs(&transaction)?;
    
        statement.finalize()?;
        info!("finished setting up database");
        
        transaction.commit()?;

        Ok(())
    }
}

fn migration_legacy_encounter(transaction: &Transaction) -> Result<(), rusqlite::Error> {
    transaction.execute_batch(
        "
    CREATE TABLE IF NOT EXISTS encounter (
        id INTEGER PRIMARY KEY,
        last_combat_packet INTEGER,
        fight_start INTEGER,
        local_player TEXT,
        current_boss TEXT,
        duration INTEGER,
        total_damage_dealt INTEGER,
        top_damage_dealt INTEGER,
        total_damage_taken INTEGER,
        top_damage_taken INTEGER,
        dps INTEGER,
        buffs TEXT,
        debuffs TEXT,
        total_shielding INTEGER DEFAULT 0,
        total_effective_shielding INTEGER DEFAULT 0,
        applied_shield_buffs TEXT,
        misc TEXT,
        difficulty TEXT,
        favorite BOOLEAN NOT NULL DEFAULT 0,
        cleared BOOLEAN,
        version INTEGER NOT NULL DEFAULT 5,
        boss_only_damage BOOLEAN NOT NULL DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS encounter_fight_start_index
    ON encounter (fight_start desc);
    CREATE INDEX IF NOT EXISTS encounter_current_boss_index
    ON encounter (current_boss);
    ")?;

    let mut stmt = transaction.prepare("SELECT 1 FROM pragma_table_info(?) WHERE name=?")?;
    if !stmt.exists(["encounter", "misc"])? {
        transaction.execute("ALTER TABLE encounter ADD COLUMN misc TEXT", [])?;
    }
    if !stmt.exists(["encounter", "difficulty"])? {
        transaction.execute("ALTER TABLE encounter ADD COLUMN difficulty TEXT", [])?;
    }
    if !stmt.exists(["encounter", "favorite"])? {
        transaction.execute_batch(
            "
            ALTER TABLE encounter ADD COLUMN favorite BOOLEAN DEFAULT 0;
            ALTER TABLE encounter ADD COLUMN version INTEGER DEFAULT {};
            ALTER TABLE encounter ADD COLUMN cleared BOOLEAN;
            ")?;
    }
    if !stmt.exists(["encounter", "boss_only_damage"])? {
        transaction.execute(
            "ALTER TABLE encounter ADD COLUMN boss_only_damage BOOLEAN NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !stmt.exists(["encounter", "total_shielding"])? {
        transaction.execute_batch(
            "
                ALTER TABLE encounter ADD COLUMN total_shielding INTEGER DEFAULT 0;
                ALTER TABLE encounter ADD COLUMN total_effective_shielding INTEGER DEFAULT 0;
                ALTER TABLE encounter ADD COLUMN applied_shield_buffs TEXT;
                ",
        )?;
    }
    transaction.execute("UPDATE encounter SET cleared = coalesce(json_extract(misc, '$.raidClear'), 0) WHERE cleared IS NULL;", [])?;
    stmt.finalize()
}

fn migration_legacy_entity(transaction: &Transaction) -> Result<(), rusqlite::Error> {
    transaction.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS entity (
            name TEXT,
            character_id INTEGER,
            encounter_id INTEGER NOT NULL,
            npc_id INTEGER,
            entity_type TEXT,
            class_id INTEGER,
            class TEXT,
            gear_score REAL,
            current_hp INTEGER,
            max_hp INTEGER,
            is_dead INTEGER,
            skills TEXT,
            damage_stats TEXT,
            dps INTEGER,
            skill_stats TEXT,
            last_update INTEGER,
            engravings TEXT,
            PRIMARY KEY (name, encounter_id),
            FOREIGN KEY (encounter_id) REFERENCES encounter (id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS entity_encounter_id_index
        ON entity (encounter_id desc);
        CREATE INDEX IF NOT EXISTS entity_name_index
        ON entity (name);
        CREATE INDEX IF NOT EXISTS entity_class_index
        ON entity (class);
        ",
    )?;

    let mut stmt = transaction.prepare("SELECT 1 FROM pragma_table_info(?) WHERE name=?")?;
    if !stmt.exists(["entity", "dps"])? {
        transaction.execute("ALTER TABLE entity ADD COLUMN dps INTEGER", [])?;
    }
    if !stmt.exists(["entity", "character_id"])? {
        transaction.execute("ALTER TABLE entity ADD COLUMN character_id INTEGER", [])?;
    }
    if !stmt.exists(["entity", "engravings"])? {
        transaction.execute("ALTER TABLE entity ADD COLUMN engravings TEXT", [])?;
    }
    if !stmt.exists(["entity", "gear_hash"])? {
        transaction.execute("ALTER TABLE entity ADD COLUMN gear_hash TEXT", [])?;
    }
    transaction.execute("UPDATE entity SET dps = coalesce(json_extract(damage_stats, '$.dps'), 0) WHERE dps IS NULL;", [])?;
    stmt.finalize()
}

fn migration_full_text_search(transaction: &Transaction) -> Result<(), rusqlite::Error> {
    transaction.execute_batch(
        "
        CREATE TABLE encounter_preview (
            id INTEGER PRIMARY KEY,
            fight_start INTEGER,
            current_boss TEXT,
            duration INTEGER,
            players TEXT,
            difficulty TEXT,
            local_player TEXT,
            my_dps INTEGER,
            favorite BOOLEAN NOT NULL DEFAULT 0,
            cleared BOOLEAN,
            boss_only_damage BOOLEAN NOT NULL DEFAULT 0,
            FOREIGN KEY (id) REFERENCES encounter(id) ON DELETE CASCADE
        );

        INSERT INTO encounter_preview SELECT
            id, fight_start, current_boss, duration, 
            (
                SELECT GROUP_CONCAT(class_id || ':' || name ORDER BY dps DESC)
                FROM entity
                WHERE encounter_id = encounter.id AND entity_type = 'PLAYER'
            ) AS players,
            difficulty, local_player,
            (
                SELECT dps
                FROM entity
                WHERE encounter_id = encounter.id AND name = encounter.local_player
            ) AS my_dps,
            favorite, cleared, boss_only_damage
        FROM encounter;

        DROP INDEX IF EXISTS encounter_fight_start_index;
        DROP INDEX IF EXISTS encounter_current_boss_index;
        DROP INDEX IF EXISTS encounter_favorite_index;
        DROP INDEX IF EXISTS entity_name_index;
        DROP INDEX IF EXISTS entity_class_index;

        ALTER TABLE encounter DROP COLUMN fight_start;
        ALTER TABLE encounter DROP COLUMN current_boss;
        ALTER TABLE encounter DROP COLUMN duration;
        ALTER TABLE encounter DROP COLUMN difficulty;
        ALTER TABLE encounter DROP COLUMN local_player;
        ALTER TABLE encounter DROP COLUMN favorite;
        ALTER TABLE encounter DROP COLUMN cleared;
        ALTER TABLE encounter DROP COLUMN boss_only_damage;

        ALTER TABLE encounter ADD COLUMN boss_hp_log BLOB;
        ALTER TABLE encounter ADD COLUMN stagger_log TEXT;

        CREATE INDEX encounter_preview_favorite_index ON encounter_preview(favorite);
        CREATE INDEX encounter_preview_fight_start_index ON encounter_preview(fight_start);
        CREATE INDEX encounter_preview_my_dps_index ON encounter_preview(my_dps);
        CREATE INDEX encounter_preview_duration_index ON encounter_preview(duration);

        CREATE VIRTUAL TABLE encounter_search USING fts5(
            current_boss, players, columnsize=0, detail=full,
            tokenize='trigram remove_diacritics 1',
            content=encounter_preview, content_rowid=id
        );
        INSERT INTO encounter_search(encounter_search) VALUES('rebuild');
        CREATE TRIGGER encounter_preview_ai AFTER INSERT ON encounter_preview BEGIN
            INSERT INTO encounter_search(rowid, current_boss, players)
            VALUES (new.id, new.current_boss, new.players);
        END;
        CREATE TRIGGER encounter_preview_ad AFTER DELETE ON encounter_preview BEGIN
            INSERT INTO encounter_search(encounter_search, rowid, current_boss, players)
            VALUES('delete', old.id, old.current_boss, old.players);
        END;
        CREATE TRIGGER encounter_preview_au AFTER UPDATE OF current_boss, players ON encounter_preview BEGIN
            INSERT INTO encounter_search(encounter_search, rowid, current_boss, players)
            VALUES('delete', old.id, old.current_boss, old.players);
            INSERT INTO encounter_search(rowid, current_boss, players)
            VALUES (new.id, new.current_boss, new.players);
        END;
        ",
    )
}

fn migration_sync(transaction: &Transaction) -> Result<(), rusqlite::Error> {
    transaction.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sync_logs (
        encounter_id INTEGER PRIMARY KEY,
        upstream_id TEXT,
        failed BOOLEAN NOT NULL DEFAULT 0,
        FOREIGN KEY (encounter_id) REFERENCES encounter (id) ON DELETE CASCADE
    );",
    )
}

fn migration_specs(transaction: &Transaction) -> Result<(), rusqlite::Error> {
    let mut stmt = transaction.prepare("SELECT 1 FROM pragma_table_info(?) WHERE name=?")?;
    if !stmt.exists(["entity", "spec"])? {
        info!("adding spec info columns");
        transaction.execute_batch(
            "
                ALTER TABLE entity ADD COLUMN spec TEXT;
                ALTER TABLE entity ADD COLUMN ark_passive_active BOOLEAN;
                ALTER TABLE entity ADD COLUMN ark_passive_data TEXT;
                ",
        )?;
    }

    stmt.finalize()
}



#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
    use crate::connection_pool;
    use super::MigrationRunner;

    fn get_semi_random_db_path() -> PathBuf {
        let path = env::current_dir().unwrap();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        path.join(format!("test_db_{}.db", timestamp))
    }
    
    #[test]
    fn should_create_new_database() {
        let connection_pool = connection_pool::in_memory();
        let migration_runner = MigrationRunner::new(connection_pool);

        migration_runner.run().unwrap();
    }
}