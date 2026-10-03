//! Non-authorizing UI preferences: no paths, credentials or consent are accepted.
use mochi_domain::ProjectId;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HomePreferences {
    pub schema_version: u8,
    pub home_reached: bool,
    pub project_id: Option<ProjectId>,
}
impl Default for HomePreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            home_reached: false,
            project_id: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferenceError {
    Unavailable,
    Invalid,
}
impl PreferenceError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "Home preferences unavailable.",
            Self::Invalid => "Home preferences invalid.",
        }
    }
}
type Result<T> = std::result::Result<T, PreferenceError>;
pub struct PreferenceStore {
    root: PathBuf,
    lock: Mutex<()>,
}
impl PreferenceStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Mutex::new(()),
        }
    }
    fn path(&self) -> PathBuf {
        self.root.join("home-preferences-v1.json")
    }
    fn private_root(&self) -> Result<()> {
        let m = fs::symlink_metadata(&self.root).map_err(|_| PreferenceError::Unavailable)?;
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(PreferenceError::Unavailable);
        }
        #[cfg(unix)]
        if m.uid() != unsafe { libc::geteuid() } || m.permissions().mode() & 0o077 != 0 {
            return Err(PreferenceError::Unavailable);
        }
        Ok(())
    }
    pub fn read(&self) -> Result<HomePreferences> {
        let _guard = self.lock.lock().map_err(|_| PreferenceError::Unavailable)?;
        self.private_root()?;
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        opts.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = match opts.open(self.path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(HomePreferences::default())
            }
            Err(_) => return Err(PreferenceError::Unavailable),
        };
        check_file(&file.metadata().map_err(|_| PreferenceError::Unavailable)?)?;
        let mut bytes = Vec::new();
        file.take(513)
            .read_to_end(&mut bytes)
            .map_err(|_| PreferenceError::Unavailable)?;
        if bytes.len() > 512 {
            return Err(PreferenceError::Invalid);
        }
        let value: HomePreferences =
            serde_json::from_slice(&bytes).map_err(|_| PreferenceError::Invalid)?;
        if value.schema_version != 1 {
            return Err(PreferenceError::Invalid);
        }
        Ok(value)
    }
    pub fn save(&self, value: &HomePreferences) -> Result<()> {
        if value.schema_version != 1 {
            return Err(PreferenceError::Invalid);
        }
        let _guard = self.lock.lock().map_err(|_| PreferenceError::Unavailable)?;
        self.private_root()?;
        match fs::symlink_metadata(self.path()) {
            Ok(m) => check_file(&m)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(PreferenceError::Unavailable),
        }
        let bytes = serde_json::to_vec(value).map_err(|_| PreferenceError::Invalid)?;
        let temp = self
            .root
            .join(format!(".home-preferences-{}.tmp", uuid::Uuid::new_v4()));
        let result = self.write_atomic(&temp, &bytes);
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    fn write_atomic(&self, temp: &Path, bytes: &[u8]) -> Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let mut file = opts.open(temp).map_err(|_| PreferenceError::Unavailable)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| PreferenceError::Unavailable)?;
        fs::rename(temp, self.path()).map_err(|_| PreferenceError::Unavailable)?;
        File::open(&self.root)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| PreferenceError::Unavailable)
    }
}
fn check_file(m: &fs::Metadata) -> Result<()> {
    if !m.is_file() || m.file_type().is_symlink() {
        return Err(PreferenceError::Unavailable);
    }
    #[cfg(unix)]
    if m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.permissions().mode() & 0o077 != 0
    {
        return Err(PreferenceError::Unavailable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (tempfile::TempDir, PreferenceStore) {
        let dir = tempfile::tempdir().expect("test directory");
        #[cfg(unix)]
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
        let store = PreferenceStore::new(dir.path().into());
        (dir, store)
    }
    #[test]
    fn preferences_survive_restart_and_contain_only_non_authorizing_metadata() {
        let (dir, store) = store();
        assert_eq!(store.read().expect("defaults"), HomePreferences::default());
        let value = HomePreferences {
            home_reached: true,
            project_id: Some(ProjectId::new()),
            ..HomePreferences::default()
        };
        store.save(&value).expect("save");
        assert_eq!(
            PreferenceStore::new(dir.path().into())
                .read()
                .expect("restart"),
            value
        );
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).expect("file")).expect("json");
        assert_eq!(json.as_object().expect("object").len(), 3);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(store.path())
                .expect("permissions")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[test]
    fn malformed_oversized_and_unknown_fields_are_rejected_without_overwriting() {
        let (dir, store) = store();
        for bytes in [
            b"{bad".as_slice(),
            br#"{"schemaVersion":1,"homeReached":true,"projectId":null,"capture":true}"#,
            &[b'x'; 513],
        ] {
            fs::write(store.path(), bytes).expect("fixture");
            #[cfg(unix)]
            fs::set_permissions(store.path(), fs::Permissions::from_mode(0o600))
                .expect("permissions");
            assert_eq!(store.read(), Err(PreferenceError::Invalid));
        }
        let prior = fs::read(store.path()).expect("prior");
        assert_eq!(
            store.save(&HomePreferences {
                schema_version: 2,
                ..HomePreferences::default()
            }),
            Err(PreferenceError::Invalid)
        );
        assert_eq!(fs::read(store.path()).expect("after"), prior);
        drop(dir);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_hardlinks_and_public_files_are_not_read_or_replaced() {
        let (dir, store) = store();
        let outside = dir.path().join("outside");
        fs::write(&outside, b"untouched").expect("fixture");
        std::os::unix::fs::symlink(&outside, store.path()).expect("symlink");
        assert!(store.read().is_err());
        assert!(store.save(&HomePreferences::default()).is_err());
        assert_eq!(fs::read(&outside).expect("outside"), b"untouched");
        fs::remove_file(store.path()).expect("remove link");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("private file");
        fs::hard_link(&outside, store.path()).expect("hardlink");
        assert!(store.read().is_err());
        assert!(store.save(&HomePreferences::default()).is_err());
        fs::remove_file(store.path()).expect("remove link");
        fs::write(store.path(), b"{}").expect("fixture");
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).expect("public file");
        assert!(store.read().is_err());
        assert!(store.save(&HomePreferences::default()).is_err());
    }
}
