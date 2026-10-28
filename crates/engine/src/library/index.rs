use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{probe_clip, thumbnail_from_file};
use crate::config::check_file_name;
use crate::error::{EngineError, Result};
use crate::paths;
use crate::record::Journal;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS clips (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    game TEXT NOT NULL,
    kind TEXT NOT NULL,
    modified_ms INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    duration_secs REAL NOT NULL,
    fps REAL NOT NULL DEFAULT -1,
    tracks TEXT NOT NULL,
    thumbnail TEXT,
    favorite INTEGER NOT NULL DEFAULT 0,
    mix TEXT
);
CREATE INDEX IF NOT EXISTS clips_by_date ON clips(modified_ms);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipKind {
    Replay,
    Recording,
}

impl ClipKind {
    fn of(name: &str) -> Self {
        let mut parts = name.rsplit('_');
        let last = parts.next().unwrap_or_default();
        let replay = last == "replay"
            || (last.chars().all(|c| c.is_ascii_digit()) && parts.next() == Some("replay"));
        if replay {
            Self::Replay
        } else {
            Self::Recording
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::Recording => "recording",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Clip {
    pub path: String,
    pub name: String,
    pub game: String,
    pub kind: ClipKind,
    pub modified_ms: u64,
    pub size_bytes: u64,
    pub duration_secs: f64,
    pub fps: f64,
    pub tracks: Vec<String>,
    pub thumbnail: Option<String>,
    pub favorite: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipSort {
    #[default]
    Newest,
    Oldest,
    Largest,
    Longest,
    Name,
}

impl ClipSort {
    fn order_by(self) -> &'static str {
        match self {
            Self::Newest => "modified_ms DESC",
            Self::Oldest => "modified_ms ASC",
            Self::Largest => "size_bytes DESC",
            Self::Longest => "duration_secs DESC",
            Self::Name => "name COLLATE NOCASE ASC",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ClipQuery {
    pub game: Option<String>,
    pub kind: Option<ClipKind>,
    pub favorites: bool,
    pub since_ms: Option<u64>,
    pub search: Option<String>,
    pub sort: ClipSort,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    pub indexed: usize,
    pub removed: usize,
}

pub struct Library {
    conn: Connection,
    root: PathBuf,
    journal: Option<Journal>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

fn db_error(error: rusqlite::Error) -> EngineError {
    EngineError::Library(error.to_string())
}

struct Scanned {
    path: PathBuf,
    modified_ms: u64,
    size_bytes: u64,
}

fn is_clip(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        && !path
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().ends_with(".part"))
}

fn scan(root: &Path) -> Vec<Scanned> {
    let dirs = std::iter::once(root.to_path_buf()).chain(
        std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path()),
    );
    let mut found = Vec::new();
    for dir in dirs {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !is_clip(&path) {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                found.push(Scanned {
                    modified_ms: modified_ms(&metadata),
                    size_bytes: metadata.len(),
                    path,
                });
            }
        }
    }
    found
}

fn modified_ms(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_millis() as u64)
}

fn key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl Library {
    pub fn open(db: &Path, root: PathBuf) -> Result<Self> {
        if let Some(dir) = db.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(db).map_err(db_error)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_error)?;
        conn.execute_batch(SCHEMA).map_err(db_error)?;
        for (column, definition) in [("fps", "REAL NOT NULL DEFAULT -1"), ("mix", "TEXT")] {
            let exists = conn
                .prepare("SELECT 1 FROM pragma_table_info('clips') WHERE name = ?1")
                .and_then(|mut statement| statement.exists([column]))
                .map_err(db_error)?;
            if !exists {
                conn.execute_batch(&format!(
                    "ALTER TABLE clips ADD COLUMN {column} {definition}"
                ))
                .map_err(db_error)?;
            }
        }
        Ok(Self {
            conn,
            root,
            journal: Journal::open_default().ok(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.root = root;
    }

    fn in_progress(&self) -> HashSet<PathBuf> {
        self.journal
            .as_ref()
            .map(|journal| {
                journal
                    .entries()
                    .into_iter()
                    .map(|(_, path)| path)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn under_root(&self, path: &Path) -> bool {
        match path.parent() {
            Some(parent) if parent == self.root => true,
            Some(parent) => parent.parent() == Some(self.root.as_path()),
            None => false,
        }
    }

    fn known(&self) -> Result<HashMap<String, (u64, u64)>> {
        let mut statement = self
            .conn
            .prepare("SELECT path, modified_ms, size_bytes, fps FROM clips")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                let probed = row.get::<_, f64>(3)? >= 0.0;
                Ok((
                    row.get::<_, String>(0)?,
                    if probed {
                        (row.get::<_, i64>(1)? as u64, row.get::<_, i64>(2)? as u64)
                    } else {
                        (0, 0)
                    },
                ))
            })
            .map_err(db_error)?;
        let mut known = HashMap::new();
        for row in rows {
            let (path, stamp) = row.map_err(db_error)?;
            if self.under_root(Path::new(&path)) {
                known.insert(path, stamp);
            }
        }
        Ok(known)
    }

    pub fn sync(&mut self) -> Result<SyncReport> {
        let mut known = self.known()?;
        let in_progress = self.in_progress();
        let mut report = SyncReport::default();
        for file in scan(&self.root) {
            let stamp = known.remove(&key(&file.path));
            if in_progress.contains(&file.path)
                || stamp == Some((file.modified_ms, file.size_bytes))
            {
                continue;
            }
            if self.index(&file)? {
                report.indexed += 1;
            }
        }
        for path in known.keys() {
            self.forget(Path::new(path))?;
            report.removed += 1;
        }
        Ok(report)
    }

    pub fn apply_changes(&mut self, paths: &[PathBuf]) -> Result<SyncReport> {
        if paths.iter().any(|path| !is_clip(path)) {
            return self.sync();
        }
        let in_progress = self.in_progress();
        let mut report = SyncReport::default();
        for path in paths {
            if in_progress.contains(path) || !self.under_root(path) {
                continue;
            }
            match std::fs::metadata(path) {
                Ok(metadata) if metadata.is_file() => {
                    let file = Scanned {
                        modified_ms: modified_ms(&metadata),
                        size_bytes: metadata.len(),
                        path: path.clone(),
                    };
                    if self.index(&file)? {
                        report.indexed += 1;
                    }
                }
                _ => {
                    if self.forget(path)? {
                        report.removed += 1;
                    }
                }
            }
        }
        Ok(report)
    }

    fn index(&self, file: &Scanned) -> Result<bool> {
        let info = match probe_clip(&file.path) {
            Ok(info) => info,
            Err(error) => {
                tracing::warn!(%error, path = %file.path.display(), "clip illisible, non indexé");
                return Ok(false);
            }
        };
        let name = file
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let game = file
            .path
            .parent()
            .filter(|parent| *parent != self.root)
            .and_then(Path::file_name)
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let thumbnail = paths::thumbnail_path(&file.path).ok().filter(|thumbnail| {
            thumbnail.exists()
                || thumbnail_from_file(&file.path, thumbnail)
                    .inspect_err(|error| {
                        tracing::debug!(%error, path = %file.path.display(), "miniature non créée");
                    })
                    .is_ok()
        });
        let thumbnail = thumbnail.map(|thumbnail| key(&thumbnail));
        let tracks = serde_json::to_string(&info.tracks)
            .map_err(|error| EngineError::Library(error.to_string()))?;
        self.conn
            .execute(
                "INSERT INTO clips (path, name, game, kind, modified_ms, size_bytes, duration_secs, tracks, thumbnail, fps)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(path) DO UPDATE SET name = ?2, game = ?3, kind = ?4, modified_ms = ?5,
                     size_bytes = ?6, duration_secs = ?7, tracks = ?8, thumbnail = ?9, fps = ?10",
                params![
                    key(&file.path),
                    name,
                    game,
                    ClipKind::of(&name).as_str(),
                    file.modified_ms as i64,
                    file.size_bytes as i64,
                    info.duration_secs,
                    tracks,
                    thumbnail,
                    info.fps,
                ],
            )
            .map_err(db_error)?;
        Ok(true)
    }

    fn forget(&self, path: &Path) -> Result<bool> {
        let removed = self
            .conn
            .execute("DELETE FROM clips WHERE path = ?1", params![key(path)])
            .map_err(db_error)?;
        Ok(removed > 0)
    }

    pub fn query(&self, query: &ClipQuery) -> Result<Vec<Clip>> {
        let sql = format!(
            "SELECT path, name, game, kind, modified_ms, size_bytes, duration_secs, tracks, thumbnail, favorite, fps
             FROM clips
             WHERE (?1 IS NULL OR game = ?1)
               AND (?2 IS NULL OR kind = ?2)
               AND (?3 = 0 OR favorite = 1)
               AND (?4 IS NULL OR modified_ms >= ?4)
               AND (?5 IS NULL OR instr(lower(name), lower(?5)) > 0 OR instr(lower(game), lower(?5)) > 0)
             ORDER BY {}
             LIMIT ?6",
            query.sort.order_by()
        );
        let mut statement = self.conn.prepare(&sql).map_err(db_error)?;
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|search| !search.is_empty());
        let rows = statement
            .query_map(
                params![
                    query.game,
                    query.kind.map(ClipKind::as_str),
                    query.favorites,
                    query.since_ms.map(|since| since as i64),
                    search,
                    query.limit.map_or(-1, i64::from),
                ],
                |row| {
                    let kind: String = row.get(3)?;
                    let tracks: String = row.get(7)?;
                    Ok(Clip {
                        path: row.get(0)?,
                        name: row.get(1)?,
                        game: row.get(2)?,
                        kind: if kind == "replay" {
                            ClipKind::Replay
                        } else {
                            ClipKind::Recording
                        },
                        modified_ms: row.get::<_, i64>(4)? as u64,
                        size_bytes: row.get::<_, i64>(5)? as u64,
                        duration_secs: row.get(6)?,
                        tracks: serde_json::from_str(&tracks).unwrap_or_default(),
                        thumbnail: row.get(8)?,
                        favorite: row.get(9)?,
                        fps: row.get(10)?,
                    })
                },
            )
            .map_err(db_error)?;
        let mut clips = Vec::new();
        for row in rows {
            let clip = row.map_err(db_error)?;
            if self.under_root(Path::new(&clip.path)) {
                clips.push(clip);
            }
        }
        Ok(clips)
    }

    pub fn games(&self) -> Result<Vec<String>> {
        let mut statement = self
            .conn
            .prepare("SELECT DISTINCT game, path FROM clips ORDER BY game COLLATE NOCASE")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_error)?;
        let mut games = Vec::new();
        for row in rows {
            let (game, path) = row.map_err(db_error)?;
            if self.under_root(Path::new(&path)) && games.last() != Some(&game) {
                games.push(game);
            }
        }
        Ok(games)
    }

    pub fn set_favorite(&self, path: &Path, favorite: bool) -> Result<()> {
        self.conn
            .execute(
                "UPDATE clips SET favorite = ?2 WHERE path = ?1",
                params![key(path), favorite],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn set_mix(&self, path: &Path, mix: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE clips SET mix = ?2 WHERE path = ?1",
                params![key(path), mix],
            )
            .map_err(db_error)?;
        Ok(())
    }

    pub fn mix(&self, path: &Path) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT mix FROM clips WHERE path = ?1",
                params![key(path)],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .flatten())
    }

    pub fn rename(&self, path: &Path, name: &str) -> Result<PathBuf> {
        let name = name.trim();
        check_file_name(name)?;
        let renamed = path.with_file_name(format!("{name}.mp4"));
        if renamed == path {
            return Ok(renamed);
        }
        if renamed.exists() {
            return Err(EngineError::ClipNameTaken(name.to_owned()));
        }
        std::fs::rename(path, &renamed)?;
        let thumbnail = match (paths::thumbnail_path(path), paths::thumbnail_path(&renamed)) {
            (Ok(old), Ok(new)) if old.exists() => {
                std::fs::rename(&old, &new).ok().map(|()| key(&new))
            }
            _ => None,
        };
        self.conn
            .execute(
                "UPDATE clips SET path = ?2, name = ?3, kind = ?4, thumbnail = ?5 WHERE path = ?1",
                params![
                    key(path),
                    key(&renamed),
                    name,
                    ClipKind::of(name).as_str(),
                    thumbnail
                ],
            )
            .map_err(db_error)?;
        Ok(renamed)
    }

    pub fn forget_deleted(&self, path: &Path) -> Result<()> {
        if let Ok(thumbnail) = paths::thumbnail_path(path) {
            std::fs::remove_file(thumbnail).ok();
        }
        self.forget(path).map(|_| ())
    }

    pub fn total_bytes(&self) -> Result<u64> {
        Ok(self
            .query(&ClipQuery::default())?
            .iter()
            .map(|clip| clip.size_bytes)
            .sum())
    }

    pub fn over_quota(&self, quota_bytes: u64) -> Result<Vec<Clip>> {
        let total = self.total_bytes()?;
        let mut excess = total.saturating_sub(quota_bytes);
        let mut victims = Vec::new();
        if excess == 0 {
            return Ok(victims);
        }
        let in_progress = self.in_progress();
        let oldest = self.query(&ClipQuery {
            sort: ClipSort::Oldest,
            ..ClipQuery::default()
        })?;
        for clip in oldest {
            if excess == 0 {
                break;
            }
            if clip.favorite || in_progress.contains(Path::new(&clip.path)) {
                continue;
            }
            excess = excess.saturating_sub(clip.size_bytes);
            victims.push(clip);
        }
        Ok(victims)
    }

    pub fn apply_quota(&self, quota_bytes: u64) -> Result<Vec<Clip>> {
        let victims = self.over_quota(quota_bytes)?;
        let mut deleted = Vec::new();
        for clip in victims {
            let path = PathBuf::from(&clip.path);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    tracing::info!(path = %path.display(), "clip supprimé par le quota");
                    self.forget_deleted(&path)?;
                    deleted.push(clip);
                }
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "clip du quota non supprimé");
                }
            }
        }
        Ok(deleted)
    }

    pub fn contains(&self, path: &Path) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM clips WHERE path = ?1",
                params![key(path)],
                |_| Ok(()),
            )
            .optional()
            .map_err(db_error)?
            .is_some()
            && self.under_root(path))
    }

    pub fn favorite(&self, path: &Path) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT favorite FROM clips WHERE path = ?1",
                params![key(path)],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use rsmpeg::avcodec::AVPacket;

    use super::*;
    use crate::encode::AacEncoder;
    use crate::mux::{Mp4Writer, MuxStream};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("palaclips-library-tests")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn write_clip(path: &Path, seconds: usize, age_days: u64) {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("dir");
        let mut encoder = AacEncoder::open(48_000, 2).expect("aac");
        let samples = vec![0.1f32; 48_000 * 2 * seconds];
        let mut packets: Vec<AVPacket> = Vec::new();
        for chunk in samples.chunks(960) {
            encoder
                .encode(chunk, &mut |packet| {
                    packets.push(packet);
                    Ok(())
                })
                .expect("encode");
        }
        let stream = MuxStream {
            codec_parameters: encoder.codec_parameters(),
            time_base: encoder.time_base(),
            frame_rate: None,
            title: Some("Mix".into()),
        };
        let mut writer = Mp4Writer::create(path, &[stream]).expect("writer");
        for packet in packets {
            writer.write(0, packet).expect("write");
        }
        writer.finish().expect("finish");
        let modified =
            std::time::SystemTime::now() - std::time::Duration::from_secs(age_days * 86_400);
        std::fs::File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(modified))
            .expect("modified");
    }

    fn library(dir: &Path) -> Library {
        let mut library = Library::open(&dir.join("library.db"), dir.join("clips")).expect("open");
        library.journal = None;
        library
    }

    fn names(clips: &[Clip]) -> Vec<&str> {
        clips.iter().map(|clip| clip.name.as_str()).collect()
    }

    #[test]
    fn replays_and_recordings_are_told_apart_by_name() {
        assert_eq!(
            ClipKind::of("Fortnite_2026-10-18_12-00-00_replay"),
            ClipKind::Replay
        );
        assert_eq!(
            ClipKind::of("Fortnite_2026-10-18_12-00-00_replay_2"),
            ClipKind::Replay
        );
        assert_eq!(
            ClipKind::of("Fortnite_2026-10-18_12-00-00"),
            ClipKind::Recording
        );
        assert_eq!(
            ClipKind::of("Fortnite_2026-10-18_12-00-00_2"),
            ClipKind::Recording
        );
    }

    #[test]
    fn sync_indexes_new_clips_forgets_vanished_ones_and_keeps_favorites() {
        let dir = scratch("sync");
        let clips = dir.join("clips");
        write_clip(&clips.join("Fortnite").join("Fortnite_a_replay.mp4"), 1, 2);
        write_clip(&clips.join("Bureau").join("Bureau_b.mp4"), 2, 1);
        let mut library = library(&dir);
        assert_eq!(
            library.sync().expect("sync"),
            SyncReport {
                indexed: 2,
                removed: 0
            }
        );
        let favorite = clips.join("Fortnite").join("Fortnite_a_replay.mp4");
        library.set_favorite(&favorite, true).expect("favorite");
        assert_eq!(
            library.sync().expect("sync"),
            SyncReport {
                indexed: 0,
                removed: 0
            }
        );
        std::fs::remove_file(clips.join("Bureau").join("Bureau_b.mp4")).expect("remove");
        assert_eq!(
            library.sync().expect("sync"),
            SyncReport {
                indexed: 0,
                removed: 1
            }
        );
        let all = library.query(&ClipQuery::default()).expect("query");
        assert_eq!(names(&all), ["Fortnite_a_replay"]);
        assert!(all[0].favorite);
        assert_eq!(all[0].game, "Fortnite");
        assert_eq!(all[0].kind, ClipKind::Replay);
        assert_eq!(all[0].tracks, ["Mix"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn queries_filter_search_and_sort() {
        let dir = scratch("query");
        let clips = dir.join("clips");
        write_clip(
            &clips.join("Fortnite").join("Fortnite_old_replay.mp4"),
            1,
            3,
        );
        write_clip(&clips.join("Fortnite").join("Fortnite_long.mp4"), 3, 2);
        write_clip(
            &clips.join("Valorant").join("Valorant_new_replay.mp4"),
            2,
            1,
        );
        let mut library = library(&dir);
        library.sync().expect("sync");
        let by = |query: ClipQuery| {
            names(&library.query(&query).expect("query"))
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            by(ClipQuery::default()),
            [
                "Valorant_new_replay",
                "Fortnite_long",
                "Fortnite_old_replay"
            ]
        );
        assert_eq!(
            by(ClipQuery {
                game: Some("Fortnite".into()),
                sort: ClipSort::Oldest,
                ..ClipQuery::default()
            }),
            ["Fortnite_old_replay", "Fortnite_long"]
        );
        assert_eq!(
            by(ClipQuery {
                kind: Some(ClipKind::Recording),
                ..ClipQuery::default()
            }),
            ["Fortnite_long"]
        );
        assert_eq!(
            by(ClipQuery {
                search: Some("VALO".into()),
                ..ClipQuery::default()
            }),
            ["Valorant_new_replay"]
        );
        assert_eq!(
            by(ClipQuery {
                sort: ClipSort::Longest,
                limit: Some(1),
                ..ClipQuery::default()
            }),
            ["Fortnite_long"]
        );
        assert_eq!(library.games().expect("games"), ["Fortnite", "Valorant"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_clip_keeps_its_player_mix_through_a_rename_and_a_rescan() {
        let dir = scratch("mix");
        let clips = dir.join("clips");
        let original = clips.join("Fortnite").join("Fortnite_a_replay.mp4");
        write_clip(&original, 1, 1);
        let mut library = library(&dir);
        library.sync().expect("sync");
        assert_eq!(library.mix(&original).expect("mix"), None);
        library.set_mix(&original, "[1]").expect("set mix");
        let renamed = library.rename(&original, "Top 1").expect("rename");
        library.sync().expect("sync");
        assert_eq!(library.mix(&renamed).expect("mix").as_deref(), Some("[1]"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_library_from_before_the_player_mix_gains_the_column() {
        let dir = scratch("old-schema");
        std::fs::create_dir_all(&dir).expect("dir");
        let db = dir.join("library.sqlite");
        Connection::open(&db)
            .and_then(|conn| {
                conn.execute_batch(
                    "CREATE TABLE clips (path TEXT PRIMARY KEY, name TEXT NOT NULL, game TEXT NOT NULL,
                     kind TEXT NOT NULL, modified_ms INTEGER NOT NULL, size_bytes INTEGER NOT NULL,
                     duration_secs REAL NOT NULL, tracks TEXT NOT NULL, thumbnail TEXT,
                     favorite INTEGER NOT NULL DEFAULT 0);",
                )
            })
            .expect("old schema");
        let library = Library::open(&db, dir.join("clips")).expect("open");
        assert_eq!(library.mix(Path::new("absent.mp4")).expect("mix"), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_renamed_clip_keeps_its_favorite_and_refuses_taken_or_invalid_names() {
        let dir = scratch("rename");
        let clips = dir.join("clips");
        let original = clips.join("Fortnite").join("Fortnite_a_replay.mp4");
        write_clip(&original, 1, 1);
        write_clip(&clips.join("Fortnite").join("Fortnite_b.mp4"), 1, 1);
        let mut library = library(&dir);
        library.sync().expect("sync");
        library.set_favorite(&original, true).expect("favorite");
        let renamed = library.rename(&original, "Top 1").expect("rename");
        assert!(renamed.exists() && !original.exists());
        assert!(library.favorite(&renamed).expect("favorite"));
        assert!(matches!(
            library.rename(&renamed, "Fortnite_b"),
            Err(EngineError::ClipNameTaken(_))
        ));
        assert!(matches!(
            library.rename(&renamed, "a:b"),
            Err(EngineError::InvalidName(..))
        ));
        assert_eq!(
            library.sync().expect("sync"),
            SyncReport {
                indexed: 0,
                removed: 0
            }
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_quota_deletes_the_oldest_clips_but_never_a_favorite() {
        let dir = scratch("quota");
        let clips = dir.join("clips");
        for (name, age) in [("a", 5), ("b", 4), ("c", 3), ("d", 2), ("e", 1)] {
            write_clip(
                &clips.join("Jeu").join(format!("Jeu_{name}_replay.mp4")),
                1,
                age,
            );
        }
        let mut library = library(&dir);
        library.sync().expect("sync");
        library
            .set_favorite(&clips.join("Jeu").join("Jeu_a_replay.mp4"), true)
            .expect("favorite");
        let size = library.query(&ClipQuery::default()).expect("query")[0].size_bytes;
        let quota = size * 3;
        assert_eq!(
            names(&library.over_quota(quota).expect("plan")),
            ["Jeu_b_replay", "Jeu_c_replay"]
        );
        assert!(library.over_quota(size * 10).expect("plan").is_empty());
        let deleted = library.apply_quota(quota).expect("apply");
        assert_eq!(deleted.len(), 2);
        assert!(!clips.join("Jeu").join("Jeu_b_replay.mp4").exists());
        assert!(clips.join("Jeu").join("Jeu_a_replay.mp4").exists());
        assert!(library.total_bytes().expect("total") <= quota);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_clip_deleted_or_renamed_on_disk_leaves_the_library_through_the_watcher() {
        let dir = scratch("watched");
        let clips = dir.join("clips");
        let first = clips.join("Jeu").join("Jeu_a_replay.mp4");
        let second = clips.join("Jeu").join("Jeu_b_replay.mp4");
        write_clip(&first, 1, 2);
        write_clip(&second, 1, 1);
        let mut library = library(&dir);
        library.sync().expect("sync");
        let (batches, received) = crossbeam_channel::unbounded();
        let _watch = crate::library::watch(&clips, move |paths| {
            batches.send(paths).ok();
        })
        .expect("watch");
        let next_batch = || {
            received
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("batch")
        };
        std::fs::remove_file(&first).expect("delete");
        let report = library.apply_changes(&next_batch()).expect("deleted");
        assert_eq!(report.removed, 1);
        let renamed = clips.join("Jeu").join("Moment.mp4");
        std::fs::rename(&second, &renamed).expect("rename");
        library.apply_changes(&next_batch()).expect("renamed");
        let all = library.query(&ClipQuery::default()).expect("query");
        assert_eq!(names(&all), ["Moment"]);
        assert!(!library.contains(&first).expect("contains"));
        assert!(!library.contains(&second).expect("contains"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_change_outside_the_clips_folder_is_ignored_and_a_folder_change_rescans() {
        let dir = scratch("changes");
        let clips = dir.join("clips");
        let mut library = library(&dir);
        write_clip(&clips.join("Jeu").join("Jeu_a.mp4"), 1, 1);
        let report = library
            .apply_changes(&[
                clips.join("Jeu").join("Jeu_a.mp4"),
                dir.join("ailleurs.mp4"),
            ])
            .expect("changes");
        assert_eq!(report.indexed, 1);
        std::fs::rename(clips.join("Jeu"), clips.join("Partie")).expect("rename folder");
        let report = library
            .apply_changes(&[clips.join("Partie")])
            .expect("rescan");
        assert_eq!(
            report,
            SyncReport {
                indexed: 1,
                removed: 1
            }
        );
        assert_eq!(library.games().expect("games"), ["Partie"]);
        std::fs::remove_dir_all(&dir).ok();
    }
}
