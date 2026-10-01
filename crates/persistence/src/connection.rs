use crate::{StorageError, StorageResult, error::map_sqlite, migrations};
use rusqlite::{Connection, OpenFlags};
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub trait Clock: Send + Sync {
    fn now_rfc3339(&self) -> StorageResult<String>;
}

pub(crate) fn utc_millis(value: &str) -> StorageResult<i64> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| StorageError::InvalidInput)?;
    if parsed.offset() != UtcOffset::UTC {
        return Err(StorageError::InvalidInput);
    }
    i64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).map_err(|_| StorageError::InvalidInput)
}

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> StorageResult<String> {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| StorageError::Serialization)
    }
}

pub struct SqliteStore {
    pub(crate) connection: Mutex<Connection>,
    pub(crate) clock: Arc<dyn Clock>,
    path: PathBuf,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>, clock: Arc<dyn Clock>) -> StorageResult<Self> {
        let path = path.as_ref().to_path_buf();
        prepare_private_file(&path)?;
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(map_sqlite)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(map_sqlite)?;
        verify_integrity(&connection)?;
        migrations::preflight(&connection)?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 PRAGMA temp_store = MEMORY;",
            )
            .map_err(map_sqlite)?;
        let now = clock.now_rfc3339()?;
        migrations::migrate(&mut connection, &now)?;
        verify_integrity(&connection)?;
        connection
            .execute_batch("PRAGMA trusted_schema = OFF;")
            .map_err(map_sqlite)?;
        secure_sidecars(&path)?;
        Ok(Self {
            connection: Mutex::new(connection),
            clock,
            path,
        })
    }

    pub fn open_system(path: impl AsRef<Path>) -> StorageResult<Self> {
        Self::open(path, Arc::new(SystemClock))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema_version(&self) -> StorageResult<i64> {
        self.lock()?
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(map_sqlite)
    }

    pub fn checkpoint(&self) -> StorageResult<()> {
        self.lock()?
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(map_sqlite)?;
        secure_sidecars(&self.path)
    }

    pub(crate) fn lock(&self) -> StorageResult<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| StorageError::Open)
    }

    pub(crate) fn now(&self) -> StorageResult<String> {
        self.clock.now_rfc3339()
    }
}

fn verify_integrity(connection: &Connection) -> StorageResult<()> {
    let result: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(map_sqlite)?;
    if result == "ok" {
        Ok(())
    } else {
        Err(StorageError::Corrupt)
    }
}

fn prepare_private_file(path: &Path) -> StorageResult<()> {
    let parent = path.parent().ok_or(StorageError::InvalidInput)?;
    fs::create_dir_all(parent).map_err(|_| StorageError::Open)?;
    #[cfg(unix)]
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|_| StorageError::Permission)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path).map_err(|_| StorageError::Open)?;
    set_private_file(path)
}

fn secure_sidecars(path: &Path) -> StorageResult<()> {
    set_private_file(path)?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        if sidecar.exists() {
            set_private_file(&sidecar)?;
        }
    }
    Ok(())
}

fn set_private_file(path: &Path) -> StorageResult<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| StorageError::Permission)?;
    Ok(())
}
