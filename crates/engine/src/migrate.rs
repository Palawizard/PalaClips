use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::{EngineError, Result};

const ROAMING_ENTRIES: [&str; 6] = [
    "settings.json",
    "settings.json.bak",
    "library.db",
    "library.db-wal",
    "library.db-shm",
    "recordings.txt",
];
const LOCAL_ENTRIES: [&str; 2] = ["cache", "replay"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirs {
    pub roaming: PathBuf,
    pub local: PathBuf,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Migration {
    pub moved: Vec<PathBuf>,
    pub kept: Vec<PathBuf>,
}

pub fn migrate_data(old: &DataDirs, new: &DataDirs) -> Result<Migration> {
    let mut migration = Migration::default();
    for entry in ROAMING_ENTRIES {
        move_entry(
            &old.roaming.join(entry),
            &new.roaming.join(entry),
            &mut migration,
        )?;
    }
    for entry in LOCAL_ENTRIES {
        move_entry(
            &old.local.join(entry),
            &new.local.join(entry),
            &mut migration,
        )?;
    }
    let database = new.roaming.join("library.db");
    if migration.moved.contains(&database) {
        rebase_thumbnails(
            &database,
            &old.local.join("cache"),
            &new.local.join("cache"),
        )?;
    }
    for dir in [&old.roaming, &old.local] {
        std::fs::remove_dir(dir).ok();
    }
    if !migration.moved.is_empty() {
        tracing::info!(
            moved = migration.moved.len(),
            kept = migration.kept.len(),
            "données déplacées vers le dossier de l'app"
        );
    }
    Ok(migration)
}

fn move_entry(from: &Path, to: &Path, migration: &mut Migration) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if from.is_dir() && to.is_dir() {
        for child in std::fs::read_dir(from)? {
            let name = child?.file_name();
            move_entry(&from.join(&name), &to.join(&name), migration)?;
        }
        std::fs::remove_dir(from).ok();
        return Ok(());
    }
    if to.exists() {
        migration.kept.push(from.to_owned());
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_err() {
        copy_all(from, to)?;
        remove_all(from)?;
    }
    migration.moved.push(to.to_owned());
    Ok(())
}

fn copy_all(from: &Path, to: &Path) -> Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for child in std::fs::read_dir(from)? {
            let name = child?.file_name();
            copy_all(&from.join(&name), &to.join(&name))?;
        }
    } else {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

fn remove_all(path: &Path) -> Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn rebase_thumbnails(database: &Path, old_cache: &Path, new_cache: &Path) -> Result<()> {
    let db_error = |error: rusqlite::Error| EngineError::Library(error.to_string());
    let connection = Connection::open(database).map_err(db_error)?;
    let has_clips: bool = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'clips')",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if has_clips {
        connection
            .execute(
                "UPDATE clips SET thumbnail = ?2 || substr(thumbnail, length(?1) + 1) WHERE substr(thumbnail, 1, length(?1)) = ?1",
                [old_cache.to_string_lossy(), new_cache.to_string_lossy()],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-migrate-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    fn write(path: &Path, content: &[u8]) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, content).expect("write");
    }

    fn dirs(root: &Path, kind: &str) -> DataDirs {
        DataDirs {
            roaming: root.join(kind).join("Roaming"),
            local: root.join(kind).join("Local"),
        }
    }

    #[test]
    fn every_file_moves_once_with_the_same_content_and_thumbnails_follow() {
        let root = scratch("full");
        let old = dirs(&root, "PalaClips");
        let new = dirs(&root, "com.palawi.palaclips");
        write(&old.roaming.join("settings.json"), b"{\"theme\":\"light\"}");
        write(&old.local.join("cache/thumbnails/clip.jpg"), b"jpeg");
        write(&old.local.join("cache/tracks/a.m4a"), &[7; 4096]);
        write(&old.local.join("replay/00000001.seg"), &[1; 1024]);
        write(&new.local.join("EBWebView/Local State"), b"webview");
        let thumbnail = old.local.join("cache/thumbnails/clip.jpg");
        {
            let connection = Connection::open(old.roaming.join("library.db")).expect("db");
            connection
                .execute_batch(
                    "CREATE TABLE clips (path TEXT PRIMARY KEY, thumbnail TEXT, favorite INTEGER)",
                )
                .expect("schema");
            connection
                .execute(
                    "INSERT INTO clips VALUES ('F:/clip.mp4', ?1, 1)",
                    [thumbnail.to_string_lossy()],
                )
                .expect("row");
        }

        let migration = migrate_data(&old, &new).expect("migration");

        assert_eq!(
            std::fs::read(new.roaming.join("settings.json")).expect("settings"),
            b"{\"theme\":\"light\"}"
        );
        assert_eq!(
            std::fs::read(new.local.join("cache/thumbnails/clip.jpg")).expect("thumb"),
            b"jpeg"
        );
        assert_eq!(
            std::fs::read(new.local.join("cache/tracks/a.m4a")).expect("track"),
            vec![7; 4096]
        );
        assert_eq!(
            std::fs::read(new.local.join("replay/00000001.seg")).expect("spool"),
            vec![1; 1024]
        );
        assert_eq!(
            std::fs::read(new.local.join("EBWebView/Local State")).expect("webview"),
            b"webview"
        );
        assert!(!old.roaming.exists() && !old.local.exists());
        assert!(migration.kept.is_empty());
        let connection = Connection::open(new.roaming.join("library.db")).expect("db");
        let (rebased, favorite): (String, i64) = connection
            .query_row("SELECT thumbnail, favorite FROM clips", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("row");
        assert_eq!(
            PathBuf::from(rebased),
            new.local.join("cache").join("thumbnails/clip.jpg")
        );
        assert_eq!(favorite, 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn newer_data_is_never_overwritten_and_a_second_run_does_nothing() {
        let root = scratch("kept");
        let old = dirs(&root, "PalaClips");
        let new = dirs(&root, "com.palawi.palaclips");
        write(&old.roaming.join("settings.json"), b"old");
        write(&new.roaming.join("settings.json"), b"new");
        let migration = migrate_data(&old, &new).expect("migration");
        assert_eq!(
            std::fs::read(new.roaming.join("settings.json")).expect("settings"),
            b"new"
        );
        assert_eq!(
            std::fs::read(old.roaming.join("settings.json")).expect("old"),
            b"old"
        );
        assert_eq!(migration.kept, [old.roaming.join("settings.json")]);
        assert!(migrate_data(&old, &new).expect("again").moved.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn nothing_to_move_leaves_everything_alone() {
        let root = scratch("empty");
        let migration = migrate_data(
            &dirs(&root, "PalaClips"),
            &dirs(&root, "com.palawi.palaclips"),
        )
        .expect("migration");
        assert_eq!(migration, Migration::default());
        assert!(!root.exists());
    }
}
