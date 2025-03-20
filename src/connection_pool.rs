use std::path::Path;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub fn get(path: &Path) -> Pool<SqliteConnectionManager> {
    let manager = SqliteConnectionManager::file(&path);
    let pool = r2d2::Pool::builder()
        .build(manager).unwrap();
    
    pool
}

#[cfg(test)]
pub fn in_memory() -> Pool<SqliteConnectionManager> {
    let manager = SqliteConnectionManager::memory();
    let pool = r2d2::Pool::builder()
        .build(manager).unwrap();
    
    pool
}