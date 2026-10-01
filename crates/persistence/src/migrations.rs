use crate::{StorageError, StorageResult, error::map_sqlite};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 5;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "preassembly_evidence",
        sql: include_str!("migrations/0001_preassembly.sql"),
    },
    Migration {
        version: 2,
        name: "validated_sessions",
        sql: include_str!("migrations/0002_sessions.sql"),
    },
    Migration {
        version: 3,
        name: "session_assembly",
        sql: include_str!("migrations/0003_session_assembly.sql"),
    },
    Migration {
        version: 4,
        name: "capture_episodes",
        sql: include_str!("migrations/0004_episodes.sql"),
    },
    Migration {
        version: 5,
        name: "learning",
        sql: include_str!("migrations/0005_learning.sql"),
    },
];

pub(crate) fn preflight(connection: &Connection) -> StorageResult<()> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if !exists {
        return Ok(());
    }
    let highest: Option<i64> = connection
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(map_sqlite)?;
    if highest.unwrap_or(0) > LATEST_SCHEMA_VERSION {
        return Err(StorageError::SchemaTooNew);
    }
    for migration in MIGRATIONS {
        let actual: Option<String> = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        if actual.is_some_and(|value| value != checksum(migration.sql)) {
            return Err(StorageError::ChecksumMismatch);
        }
    }
    Ok(())
}

pub(crate) fn migrate(connection: &mut Connection, now: &str) -> StorageResult<()> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )
        .map_err(|_| StorageError::Migration)?;

    preflight(connection)?;

    for migration in MIGRATIONS {
        let expected = checksum(migration.sql);
        let existing: Option<String> = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [migration.version],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        match existing {
            Some(actual) if actual != expected => return Err(StorageError::ChecksumMismatch),
            Some(_) => continue,
            None => {}
        }

        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| StorageError::Migration)?;
        transaction
            .execute_batch(migration.sql)
            .map_err(|_| StorageError::Migration)?;
        transaction
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![migration.version, migration.name, expected, now],
            )
            .map_err(|_| StorageError::Migration)?;
        transaction.commit().map_err(|_| StorageError::Migration)?;
    }
    Ok(())
}

fn checksum(sql: &str) -> String {
    format!("{:x}", Sha256::digest(sql.as_bytes()))
}
