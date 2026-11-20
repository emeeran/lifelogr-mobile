//! LifeLogr's local store: one SQLite file (WAL + FTS5) plus a media folder.
//!
//! Everything here is plain Rust with no Tauri types, so it is unit-testable and
//! can later be reused by a sync service or a CLI (the PRD's "Rust core" end-state).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub type Result<T> = std::result::Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const SCHEMA_VERSION: i64 = 1;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS entries (
  id          TEXT PRIMARY KEY,
  entry_date  TEXT NOT NULL,              -- YYYY-MM-DD (local date the entry is about)
  title       TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',   -- markdown
  favorite    INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  deleted_at  TEXT                         -- soft delete = trash
);
CREATE INDEX IF NOT EXISTS idx_entries_date ON entries(entry_date DESC, created_at DESC);

CREATE TABLE IF NOT EXISTS folders (
  id          TEXT PRIMARY KEY,
  parent_id   TEXT REFERENCES folders(id),
  name        TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  id          TEXT PRIMARY KEY,
  folder_id   TEXT REFERENCES folders(id),
  title       TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',
  pinned      INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  deleted_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_notes_folder ON notes(folder_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS tags (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE COLLATE NOCASE
);
CREATE TABLE IF NOT EXISTS item_tags (
  item_kind TEXT NOT NULL,                 -- 'entry' | 'note'
  item_id   TEXT NOT NULL,
  tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (item_kind, item_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_item_tags_tag ON item_tags(tag_id);

CREATE TABLE IF NOT EXISTS media (
  id          TEXT PRIMARY KEY,
  item_kind   TEXT NOT NULL,
  item_id     TEXT NOT NULL,
  kind        TEXT NOT NULL,               -- image | video | audio | file
  filename    TEXT NOT NULL,
  mime        TEXT NOT NULL,
  size        INTEGER NOT NULL,
  file        TEXT NOT NULL,               -- path relative to media dir
  thumb       TEXT,                        -- path relative to media dir, images only
  created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_item ON media(item_kind, item_id);

CREATE TABLE IF NOT EXISTS reminders (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  body        TEXT NOT NULL DEFAULT '',
  next_at     TEXT NOT NULL,               -- RFC3339 UTC
  repeat      TEXT NOT NULL DEFAULT 'none',-- none | daily | weekly | monthly
  enabled     INTEGER NOT NULL DEFAULT 1,
  last_fired_at TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

-- One FTS index over entries and notes. Kept in sync by triggers so no code path can forget.
CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(
  kind UNINDEXED, ref_id UNINDEXED, title, body,
  tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search(kind, ref_id, title, body) VALUES ('entry', new.id, new.title, new.body);
END;
CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE ON entries BEGIN
  DELETE FROM search WHERE kind = 'entry' AND ref_id = old.id;
  INSERT INTO search(kind, ref_id, title, body)
    SELECT 'entry', new.id, new.title, new.body WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
  DELETE FROM search WHERE kind = 'entry' AND ref_id = old.id;
END;
CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes WHEN new.deleted_at IS NULL BEGIN
  INSERT INTO search(kind, ref_id, title, body) VALUES ('note', new.id, new.title, new.body);
END;
CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
  DELETE FROM search WHERE kind = 'note' AND ref_id = old.id;
  INSERT INTO search(kind, ref_id, title, body)
    SELECT 'note', new.id, new.title, new.body WHERE new.deleted_at IS NULL;
END;
CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
  DELETE FROM search WHERE kind = 'note' AND ref_id = old.id;
END;
"#;

// ---------- DTOs ----------

#[derive(Debug, Serialize, Clone)]
pub struct Media {
    pub id: String,
    pub item_kind: String,
    pub item_id: String,
    pub kind: String,
    pub filename: String,
    pub mime: String,
    pub size: i64,
    /// Absolute path; the UI turns it into a URL with `convertFileSrc`.
    pub path: String,
    pub thumb_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    pub id: String,
    pub entry_date: String,
    pub title: String,
    pub body: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub media: Vec<Media>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EntrySummary {
    pub id: String,
    pub entry_date: String,
    pub title: String,
    pub excerpt: String,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub media_count: i64,
    pub cover_thumb: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct EntryInput {
    pub id: Option<String>,
    pub entry_date: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct EntryQuery {
    pub tag: Option<String>,
    #[serde(default)]
    pub favorites_only: bool,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub note_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct FolderInput {
    pub id: Option<String>,
    pub parent_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Note {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub media: Vec<Media>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NoteSummary {
    pub id: String,
    pub folder_id: Option<String>,
    pub title: String,
    pub excerpt: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub id: Option<String>,
    pub folder_id: Option<String>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Serialize)]
pub struct Tag {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub date: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reminder {
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub next_at: String,
    #[serde(default = "default_repeat")]
    pub repeat: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub last_fired_at: Option<String>,
}
fn default_repeat() -> String {
    "none".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct TrashItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub entries: i64,
    pub notes: i64,
    pub media: i64,
    pub media_bytes: i64,
    pub db_bytes: i64,
    pub data_dir: String,
}

pub struct NewMedia<'a> {
    pub item_kind: &'a str,
    pub item_id: &'a str,
    pub filename: &'a str,
    pub mime: &'a str,
    pub bytes: &'a [u8],
}

// ---------- helpers ----------

fn now() -> String {
    // RFC3339 UTC with millisecond precision, computed by SQLite-independent code.
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format_unix_ms(d.as_millis() as i64)
}

/// Format unix milliseconds as `YYYY-MM-DDTHH:MM:SS.mmmZ` without pulling in chrono.
pub fn format_unix_ms(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let millis = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y,
        m,
        d,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60,
        millis
    )
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn excerpt(body: &str, n: usize) -> String {
    let flat: String = body
        .lines()
        .map(|l| {
            l.trim_start_matches(['#', '>', '-', '*', '+', ' '])
                .trim_start_matches("[ ] ")
                .trim_start_matches("[x] ")
                .replace(['*', '_', '`'], "")
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() > n {
        let mut s: String = flat.chars().take(n).collect();
        s.push('…');
        s
    } else {
        flat
    }
}

/// `#hashtags` written inline in a note body (letters, digits, `_`, `-`, `/`).
pub fn extract_hashtags(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut in_code = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            in_code = !in_code;
        }
        let prev_ok = i == 0 || chars[i - 1].is_whitespace() || "([{,".contains(chars[i - 1]);
        if !in_code && c == '#' && prev_ok {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_alphanumeric() || chars[j] == '_' || chars[j] == '-' || chars[j] == '/')
            {
                j += 1;
            }
            // Require at least one letter so "#1" / markdown headings ("# ") are not tags.
            let tag: String = chars[i + 1..j].iter().collect();
            if tag.chars().any(|c| c.is_alphabetic())
                && !out.iter().any(|t| t.eq_ignore_ascii_case(&tag))
            {
                out.push(tag);
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// Turn free text into a safe FTS5 query: every token quoted, last one prefix-matched.
pub fn fts_query(q: &str) -> Option<String> {
    let toks: Vec<String> = q
        .split_whitespace()
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .collect();
    if toks.is_empty() {
        return None;
    }
    let n = toks.len();
    Some(
        toks.iter()
            .enumerate()
            .map(|(i, t)| if i == n - 1 { format!("\"{t}\"*") } else { format!("\"{t}\"") })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn media_kind(mime: &str) -> &'static str {
    if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        "audio"
    } else {
        "file"
    }
}

fn safe_ext(filename: &str, mime: &str) -> String {
    let from_name = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    from_name.unwrap_or_else(|| {
        match mime.split(';').next().unwrap_or("") {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            "audio/webm" => "webm",
            "audio/ogg" => "ogg",
            "audio/mp4" => "m4a",
            "audio/mpeg" => "mp3",
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            _ => "bin",
        }
        .to_string()
    })
}

// ---------- store ----------

pub struct Store {
    conn: Mutex<Connection>,
    data_dir: PathBuf,
    media_dir: PathBuf,
}

impl Store {
    pub fn open(data_dir: &Path) -> Result<Self> {
        let media_dir = data_dir.join("media");
        fs::create_dir_all(&media_dir).map_err(err)?;
        let conn = Connection::open(data_dir.join("lifelogr.db")).map_err(err)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA cache_size = -4000;  -- ~4 MB page cache: keep memory small
             PRAGMA temp_store = MEMORY;",
        )
        .map_err(err)?;
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
        if v < 1 {
            conn.execute_batch(MIGRATION_1).map_err(err)?;
        }
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .map_err(err)?;
        Ok(Self { conn: Mutex::new(conn), data_dir: data_dir.to_path_buf(), media_dir })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn abs(&self, rel: &str) -> String {
        self.media_dir.join(rel).to_string_lossy().into_owned()
    }

    // ----- tags -----

    fn tags_for(conn: &Connection, kind: &str, id: &str) -> Result<Vec<String>> {
        let mut st = conn
            .prepare_cached(
                "SELECT t.name FROM item_tags it JOIN tags t ON t.id = it.tag_id
                 WHERE it.item_kind = ?1 AND it.item_id = ?2 ORDER BY t.name COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = st
            .query_map(params![kind, id], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    fn set_tags(conn: &Connection, kind: &str, id: &str, tags: &[String]) -> Result<()> {
        conn.execute("DELETE FROM item_tags WHERE item_kind = ?1 AND item_id = ?2", params![kind, id])
            .map_err(err)?;
        for t in tags {
            let name = t.trim().trim_start_matches('#').trim();
            if name.is_empty() {
                continue;
            }
            conn.execute("INSERT OR IGNORE INTO tags(name) VALUES (?1)", [name]).map_err(err)?;
            let tag_id: i64 = conn
                .query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get(0))
                .map_err(err)?;
            conn.execute(
                "INSERT OR IGNORE INTO item_tags(item_kind, item_id, tag_id) VALUES (?1, ?2, ?3)",
                params![kind, id, tag_id],
            )
            .map_err(err)?;
        }
        // Drop tags nobody uses any more.
        conn.execute("DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM item_tags)", [])
            .map_err(err)?;
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>> {
        let conn = self.db();
        let mut st = conn
            .prepare(
                "SELECT t.name, COUNT(*) FROM tags t JOIN item_tags it ON it.tag_id = t.id
                 LEFT JOIN entries e ON it.item_kind = 'entry' AND e.id = it.item_id
                 LEFT JOIN notes n ON it.item_kind = 'note' AND n.id = it.item_id
                 WHERE COALESCE(e.deleted_at, n.deleted_at) IS NULL
                   AND (e.id IS NOT NULL OR n.id IS NOT NULL)
                 GROUP BY t.id ORDER BY COUNT(*) DESC, t.name COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = st
            .query_map([], |r| Ok(Tag { name: r.get(0)?, count: r.get(1)? }))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    // ----- media -----

    fn media_for(&self, conn: &Connection, kind: &str, id: &str) -> Result<Vec<Media>> {
        let mut st = conn
            .prepare_cached(
                "SELECT id, item_kind, item_id, kind, filename, mime, size, file, thumb, created_at
                 FROM media WHERE item_kind = ?1 AND item_id = ?2 ORDER BY created_at",
            )
            .map_err(err)?;
        let rows = st
            .query_map(params![kind, id], |r| {
                Ok(Media {
                    id: r.get(0)?,
                    item_kind: r.get(1)?,
                    item_id: r.get(2)?,
                    kind: r.get(3)?,
                    filename: r.get(4)?,
                    mime: r.get(5)?,
                    size: r.get(6)?,
                    path: self.abs(&r.get::<_, String>(7)?),
                    thumb_path: r.get::<_, Option<String>>(8)?.map(|t| self.abs(&t)),
                    created_at: r.get(9)?,
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn add_media(&self, m: NewMedia) -> Result<Media> {
        if m.item_kind != "entry" && m.item_kind != "note" {
            return Err("item_kind must be 'entry' or 'note'".into());
        }
        let id = new_id();
        let ext = safe_ext(m.filename, m.mime);
        let kind = media_kind(m.mime);
        // Shard by first two hex chars so no single directory grows huge.
        let shard = &id[..2];
        fs::create_dir_all(self.media_dir.join(shard)).map_err(err)?;
        let rel = format!("{shard}/{id}.{ext}");
        fs::write(self.media_dir.join(&rel), m.bytes).map_err(err)?;

        // Thumbnails keep list/gallery memory low (the PRD's "memory discipline" rule).
        let thumb_rel = if kind == "image" {
            let t = format!("{shard}/{id}.thumb.jpg");
            match make_thumb(m.bytes, &self.media_dir.join(&t)) {
                Ok(()) => Some(t),
                Err(_) => None, // e.g. HEIC: fall back to the original
            }
        } else {
            None
        };

        let created = now();
        let res = {
            let conn = self.db();
            conn.execute(
                "INSERT INTO media(id, item_kind, item_id, kind, filename, mime, size, file, thumb, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, m.item_kind, m.item_id, kind, m.filename, m.mime, m.bytes.len() as i64, rel, thumb_rel, created],
            )
        };
        if let Err(e) = res {
            // DB failed: remove the orphaned files we just wrote.
            let _ = fs::remove_file(self.media_dir.join(&rel));
            if let Some(t) = &thumb_rel {
                let _ = fs::remove_file(self.media_dir.join(t));
            }
            return Err(err(e));
        }
        Ok(Media {
            id,
            item_kind: m.item_kind.into(),
            item_id: m.item_id.into(),
            kind: kind.into(),
            filename: m.filename.into(),
            mime: m.mime.into(),
            size: m.bytes.len() as i64,
            path: self.abs(&rel),
            thumb_path: thumb_rel.map(|t| self.abs(&t)),
            created_at: created,
        })
    }

    /// Deletes the DB row first, then the files (never unlink before the commit succeeds).
    pub fn delete_media(&self, id: &str) -> Result<()> {
        let files: Option<(String, Option<String>)> = {
            let conn = self.db();
            let f = conn
                .query_row("SELECT file, thumb FROM media WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()
                .map_err(err)?;
            conn.execute("DELETE FROM media WHERE id = ?1", [id]).map_err(err)?;
            f
        };
        if let Some((file, thumb)) = files {
            let _ = fs::remove_file(self.media_dir.join(file));
            if let Some(t) = thumb {
                let _ = fs::remove_file(self.media_dir.join(t));
            }
        }
        Ok(())
    }

    // ----- entries -----

    pub fn list_entries(&self, q: &EntryQuery) -> Result<Vec<EntrySummary>> {
        let conn = self.db();
        let mut sql = String::from(
            "SELECT e.id, e.entry_date, e.title, e.body, e.favorite, e.updated_at,
                    (SELECT COUNT(*) FROM media m WHERE m.item_kind = 'entry' AND m.item_id = e.id),
                    (SELECT m.thumb FROM media m WHERE m.item_kind = 'entry' AND m.item_id = e.id
                       AND m.thumb IS NOT NULL ORDER BY m.created_at LIMIT 1)
             FROM entries e WHERE e.deleted_at IS NULL",
        );
        if q.favorites_only {
            sql.push_str(" AND e.favorite = 1");
        }
        if q.tag.is_some() {
            sql.push_str(
                " AND e.id IN (SELECT it.item_id FROM item_tags it JOIN tags t ON t.id = it.tag_id
                   WHERE it.item_kind = 'entry' AND t.name = ?1)",
            );
        } else {
            sql.push_str(" AND ?1 IS NULL");
        }
        sql.push_str(" ORDER BY e.entry_date DESC, e.created_at DESC LIMIT ?2 OFFSET ?3");
        let mut st = conn.prepare(&sql).map_err(err)?;
        let rows = st
            .query_map(
                params![q.tag, q.limit.unwrap_or(50).clamp(1, 500), q.offset.unwrap_or(0).max(0)],
                |r| {
                    let body: String = r.get(3)?;
                    Ok(EntrySummary {
                        id: r.get(0)?,
                        entry_date: r.get(1)?,
                        title: r.get(2)?,
                        excerpt: excerpt(&body, 160),
                        favorite: r.get::<_, i64>(4)? != 0,
                        updated_at: r.get(5)?,
                        media_count: r.get(6)?,
                        cover_thumb: r.get::<_, Option<String>>(7)?.map(|t| self.abs(&t)),
                        tags: vec![],
                    })
                },
            )
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        let mut rows = rows;
        for r in rows.iter_mut() {
            r.tags = Self::tags_for(&conn, "entry", &r.id)?;
        }
        Ok(rows)
    }

    pub fn get_entry(&self, id: &str) -> Result<Entry> {
        let conn = self.db();
        let mut e = conn
            .query_row(
                "SELECT id, entry_date, title, body, favorite, created_at, updated_at, deleted_at
                 FROM entries WHERE id = ?1",
                [id],
                |r| {
                    Ok(Entry {
                        id: r.get(0)?,
                        entry_date: r.get(1)?,
                        title: r.get(2)?,
                        body: r.get(3)?,
                        favorite: r.get::<_, i64>(4)? != 0,
                        created_at: r.get(5)?,
                        updated_at: r.get(6)?,
                        deleted_at: r.get(7)?,
                        tags: vec![],
                        media: vec![],
                    })
                },
            )
            .optional()
            .map_err(err)?
            .ok_or("Entry not found")?;
        e.tags = Self::tags_for(&conn, "entry", id)?;
        e.media = self.media_for(&conn, "entry", id)?;
        Ok(e)
    }

    pub fn save_entry(&self, input: EntryInput) -> Result<Entry> {
        let id = {
            let mut conn = self.db();
            let tx = conn.transaction().map_err(err)?;
            let ts = now();
            let id = match &input.id {
                Some(id) => {
                    let n = tx
                        .execute(
                            "UPDATE entries SET entry_date = ?2, title = ?3, body = ?4, favorite = ?5, updated_at = ?6
                             WHERE id = ?1",
                            params![id, input.entry_date, input.title, input.body, input.favorite as i64, ts],
                        )
                        .map_err(err)?;
                    if n == 0 {
                        // Client-generated id for a brand-new entry (lets drafts attach media first).
                        tx.execute(
                            "INSERT INTO entries(id, entry_date, title, body, favorite, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                            params![id, input.entry_date, input.title, input.body, input.favorite as i64, ts],
                        )
                        .map_err(err)?;
                    }
                    id.clone()
                }
                None => {
                    let id = new_id();
                    tx.execute(
                        "INSERT INTO entries(id, entry_date, title, body, favorite, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                        params![id, input.entry_date, input.title, input.body, input.favorite as i64, ts],
                    )
                    .map_err(err)?;
                    id
                }
            };
            Self::set_tags(&tx, "entry", &id, &input.tags)?;
            tx.commit().map_err(err)?;
            id
        };
        self.get_entry(&id)
    }

    pub fn set_favorite(&self, id: &str, favorite: bool) -> Result<()> {
        self.db()
            .execute(
                "UPDATE entries SET favorite = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, favorite as i64, now()],
            )
            .map_err(err)?;
        Ok(())
    }

    // ----- folders & notes -----

    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let conn = self.db();
        let mut st = conn
            .prepare(
                "SELECT f.id, f.parent_id, f.name,
                        (SELECT COUNT(*) FROM notes n WHERE n.folder_id = f.id AND n.deleted_at IS NULL)
                 FROM folders f ORDER BY f.name COLLATE NOCASE",
            )
            .map_err(err)?;
        let rows = st
            .query_map([], |r| {
                Ok(Folder { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)?, note_count: r.get(3)? })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn save_folder(&self, input: FolderInput) -> Result<String> {
        let conn = self.db();
        let name = input.name.trim();
        if name.is_empty() {
            return Err("Folder name is required".into());
        }
        let ts = now();
        match input.id {
            Some(id) => {
                // Acyclic guard: the new parent must not be this folder or one of its descendants.
                if let Some(p) = &input.parent_id {
                    let mut cur = Some(p.clone());
                    while let Some(c) = cur {
                        if c == id {
                            return Err("A folder cannot be moved inside itself".into());
                        }
                        cur = conn
                            .query_row("SELECT parent_id FROM folders WHERE id = ?1", [&c], |r| r.get(0))
                            .optional()
                            .map_err(err)?
                            .flatten();
                    }
                }
                conn.execute(
                    "UPDATE folders SET name = ?2, parent_id = ?3, updated_at = ?4 WHERE id = ?1",
                    params![id, name, input.parent_id, ts],
                )
                .map_err(err)?;
                Ok(id)
            }
            None => {
                let id = new_id();
                conn.execute(
                    "INSERT INTO folders(id, parent_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, input.parent_id, name, ts],
                )
                .map_err(err)?;
                Ok(id)
            }
        }
    }

    /// Removes a folder; its notes and sub-folders move up to the parent (nothing is lost).
    pub fn delete_folder(&self, id: &str) -> Result<()> {
        let mut conn = self.db();
        let tx = conn.transaction().map_err(err)?;
        let parent: Option<String> = tx
            .query_row("SELECT parent_id FROM folders WHERE id = ?1", [id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .flatten();
        tx.execute("UPDATE notes SET folder_id = ?2 WHERE folder_id = ?1", params![id, parent])
            .map_err(err)?;
        tx.execute("UPDATE folders SET parent_id = ?2 WHERE parent_id = ?1", params![id, parent])
            .map_err(err)?;
        tx.execute("DELETE FROM folders WHERE id = ?1", [id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }

    /// `folder_id`: None = all notes, Some("") = unfiled, Some(id) = that folder.
    pub fn list_notes(&self, folder_id: Option<String>, tag: Option<String>) -> Result<Vec<NoteSummary>> {
        let conn = self.db();
        let mut st = conn
            .prepare(
                "SELECT n.id, n.folder_id, n.title, n.body, n.pinned, n.updated_at FROM notes n
                 WHERE n.deleted_at IS NULL
                   AND (?1 IS NULL OR (?1 = '' AND n.folder_id IS NULL) OR n.folder_id = ?1)
                   AND (?2 IS NULL OR n.id IN (SELECT it.item_id FROM item_tags it JOIN tags t ON t.id = it.tag_id
                                               WHERE it.item_kind = 'note' AND t.name = ?2))
                 ORDER BY n.pinned DESC, n.updated_at DESC",
            )
            .map_err(err)?;
        let mut rows = st
            .query_map(params![folder_id, tag], |r| {
                let body: String = r.get(3)?;
                Ok(NoteSummary {
                    id: r.get(0)?,
                    folder_id: r.get(1)?,
                    title: r.get(2)?,
                    excerpt: excerpt(&body, 120),
                    pinned: r.get::<_, i64>(4)? != 0,
                    updated_at: r.get(5)?,
                    tags: vec![],
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        for r in rows.iter_mut() {
            r.tags = Self::tags_for(&conn, "note", &r.id)?;
        }
        Ok(rows)
    }

    pub fn get_note(&self, id: &str) -> Result<Note> {
        let conn = self.db();
        let mut n = conn
            .query_row(
                "SELECT id, folder_id, title, body, pinned, created_at, updated_at, deleted_at FROM notes WHERE id = ?1",
                [id],
                |r| {
                    Ok(Note {
                        id: r.get(0)?,
                        folder_id: r.get(1)?,
                        title: r.get(2)?,
                        body: r.get(3)?,
                        pinned: r.get::<_, i64>(4)? != 0,
                        created_at: r.get(5)?,
                        updated_at: r.get(6)?,
                        deleted_at: r.get(7)?,
                        tags: vec![],
                        media: vec![],
                    })
                },
            )
            .optional()
            .map_err(err)?
            .ok_or("Note not found")?;
        n.tags = Self::tags_for(&conn, "note", id)?;
        n.media = self.media_for(&conn, "note", id)?;
        Ok(n)
    }

    pub fn save_note(&self, input: NoteInput) -> Result<Note> {
        let id = {
            let mut conn = self.db();
            let tx = conn.transaction().map_err(err)?;
            let ts = now();
            let folder = input.folder_id.filter(|f| !f.is_empty());
            let id = input.id.clone().unwrap_or_else(new_id);
            let n = tx
                .execute(
                    "UPDATE notes SET folder_id = ?2, title = ?3, body = ?4, pinned = ?5, updated_at = ?6 WHERE id = ?1",
                    params![id, folder, input.title, input.body, input.pinned as i64, ts],
                )
                .map_err(err)?;
            if n == 0 {
                tx.execute(
                    "INSERT INTO notes(id, folder_id, title, body, pinned, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    params![id, folder, input.title, input.body, input.pinned as i64, ts],
                )
                .map_err(err)?;
            }
            // Notes use the #hashtag-in-text convention.
            Self::set_tags(&tx, "note", &id, &extract_hashtags(&input.body))?;
            tx.commit().map_err(err)?;
            id
        };
        self.get_note(&id)
    }

    // ----- trash (shared by entries and notes) -----

    fn table(kind: &str) -> Result<&'static str> {
        match kind {
            "entry" => Ok("entries"),
            "note" => Ok("notes"),
            _ => Err("kind must be 'entry' or 'note'".into()),
        }
    }

    pub fn trash(&self, kind: &str, id: &str) -> Result<()> {
        let t = Self::table(kind)?;
        let ts = now();
        self.db()
            .execute(&format!("UPDATE {t} SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1"), params![id, ts])
            .map_err(err)?;
        Ok(())
    }

    pub fn restore(&self, kind: &str, id: &str) -> Result<()> {
        let t = Self::table(kind)?;
        self.db()
            .execute(&format!("UPDATE {t} SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1"), params![id, now()])
            .map_err(err)?;
        Ok(())
    }

    /// Permanently deletes an item: DB rows in one transaction, then media files.
    pub fn purge(&self, kind: &str, id: &str) -> Result<()> {
        let t = Self::table(kind)?;
        let files: Vec<(String, Option<String>)> = {
            let mut conn = self.db();
            let tx = conn.transaction().map_err(err)?;
            let files = {
                let mut st = tx
                    .prepare("SELECT file, thumb FROM media WHERE item_kind = ?1 AND item_id = ?2")
                    .map_err(err)?;
                let v = st
                    .query_map(params![kind, id], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(err)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(err)?;
                v
            };
            tx.execute("DELETE FROM media WHERE item_kind = ?1 AND item_id = ?2", params![kind, id])
                .map_err(err)?;
            Self::set_tags(&tx, kind, id, &[])?;
            tx.execute(&format!("DELETE FROM {t} WHERE id = ?1"), [id]).map_err(err)?;
            tx.commit().map_err(err)?;
            files
        };
        for (f, th) in files {
            let _ = fs::remove_file(self.media_dir.join(f));
            if let Some(th) = th {
                let _ = fs::remove_file(self.media_dir.join(th));
            }
        }
        Ok(())
    }

    pub fn list_trash(&self) -> Result<Vec<TrashItem>> {
        let conn = self.db();
        let mut st = conn
            .prepare(
                "SELECT 'entry', id, CASE WHEN title = '' THEN entry_date ELSE title END, deleted_at
                   FROM entries WHERE deleted_at IS NOT NULL
                 UNION ALL
                 SELECT 'note', id, CASE WHEN title = '' THEN 'Untitled note' ELSE title END, deleted_at
                   FROM notes WHERE deleted_at IS NOT NULL
                 ORDER BY 4 DESC",
            )
            .map_err(err)?;
        let rows = st
            .query_map([], |r| Ok(TrashItem { kind: r.get(0)?, id: r.get(1)?, title: r.get(2)?, deleted_at: r.get(3)? }))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    // ----- search -----

    pub fn search(&self, q: &str, limit: i64) -> Result<Vec<SearchHit>> {
        let Some(fq) = fts_query(q) else { return Ok(vec![]) };
        let conn = self.db();
        let mut st = conn
            .prepare(
                "SELECT s.kind, s.ref_id, s.title,
                        snippet(search, 3, '\u{1}', '\u{2}', '…', 14),
                        COALESCE(e.entry_date, substr(n.updated_at, 1, 10))
                 FROM search s
                 LEFT JOIN entries e ON s.kind = 'entry' AND e.id = s.ref_id
                 LEFT JOIN notes n ON s.kind = 'note' AND n.id = s.ref_id
                 WHERE search MATCH ?1
                 ORDER BY bm25(search, 0, 0, 5.0, 1.0) LIMIT ?2",
            )
            .map_err(err)?;
        let rows = st
            .query_map(params![fq, limit.clamp(1, 200)], |r| {
                Ok(SearchHit {
                    kind: r.get(0)?,
                    id: r.get(1)?,
                    title: r.get(2)?,
                    snippet: r.get(3)?,
                    date: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    // ----- reminders -----

    pub fn list_reminders(&self) -> Result<Vec<Reminder>> {
        let conn = self.db();
        let mut st = conn
            .prepare("SELECT id, title, body, next_at, repeat, enabled, last_fired_at FROM reminders ORDER BY next_at")
            .map_err(err)?;
        let rows = st
            .query_map([], |r| {
                Ok(Reminder {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    body: r.get(2)?,
                    next_at: r.get(3)?,
                    repeat: r.get(4)?,
                    enabled: r.get::<_, i64>(5)? != 0,
                    last_fired_at: r.get(6)?,
                })
            })
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(rows)
    }

    pub fn save_reminder(&self, r: Reminder) -> Result<Reminder> {
        if !matches!(r.repeat.as_str(), "none" | "daily" | "weekly" | "monthly") {
            return Err("repeat must be none, daily, weekly or monthly".into());
        }
        if r.title.trim().is_empty() {
            return Err("Reminder title is required".into());
        }
        let conn = self.db();
        let ts = now();
        let id = r.id.clone().unwrap_or_else(new_id);
        let n = conn
            .execute(
                "UPDATE reminders SET title = ?2, body = ?3, next_at = ?4, repeat = ?5, enabled = ?6,
                   last_fired_at = ?7, updated_at = ?8 WHERE id = ?1",
                params![id, r.title, r.body, r.next_at, r.repeat, r.enabled as i64, r.last_fired_at, ts],
            )
            .map_err(err)?;
        if n == 0 {
            conn.execute(
                "INSERT INTO reminders(id, title, body, next_at, repeat, enabled, last_fired_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![id, r.title, r.body, r.next_at, r.repeat, r.enabled as i64, r.last_fired_at, ts],
            )
            .map_err(err)?;
        }
        Ok(Reminder { id: Some(id), ..r })
    }

    pub fn delete_reminder(&self, id: &str) -> Result<()> {
        self.db().execute("DELETE FROM reminders WHERE id = ?1", [id]).map_err(err)?;
        Ok(())
    }

    // ----- stats -----

    pub fn stats(&self) -> Result<Stats> {
        let conn = self.db();
        let one = |sql: &str| -> Result<i64> { conn.query_row(sql, [], |r| r.get(0)).map_err(err) };
        let db_bytes = ["lifelogr.db", "lifelogr.db-wal"]
            .iter()
            .filter_map(|f| fs::metadata(self.data_dir.join(f)).ok())
            .map(|m| m.len() as i64)
            .sum();
        Ok(Stats {
            entries: one("SELECT COUNT(*) FROM entries WHERE deleted_at IS NULL")?,
            notes: one("SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL")?,
            media: one("SELECT COUNT(*) FROM media")?,
            media_bytes: one("SELECT COALESCE(SUM(size), 0) FROM media")?,
            db_bytes,
            data_dir: self.data_dir.to_string_lossy().into_owned(),
        })
    }
}

fn make_thumb(bytes: &[u8], out: &Path) -> std::result::Result<(), String> {
    let img = image::load_from_memory(bytes).map_err(err)?;
    let t = img.thumbnail(480, 480).to_rgb8();
    t.save_with_format(out, image::ImageFormat::Jpeg).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        (dir, s)
    }

    fn entry(title: &str, body: &str, tags: &[&str]) -> EntryInput {
        EntryInput {
            id: None,
            entry_date: "2026-11-20".into(),
            title: title.into(),
            body: body.into(),
            favorite: false,
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn time_format() {
        assert_eq!(format_unix_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(format_unix_ms(1_795_000_000_123), "2026-11-18T11:06:40.123Z");
    }

    #[test]
    fn entry_crud_tags_and_search() {
        let (_d, s) = store();
        let e = s.save_entry(entry("Hike", "Walked up the **ridge** at dawn", &["outdoors", "#Morning"])).unwrap();
        assert_eq!(e.tags, vec!["Morning", "outdoors"]);
        let hits = s.search("ridg", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, e.id);
        assert!(hits[0].snippet.contains('\u{1}'));

        let e2 = s
            .save_entry(EntryInput { id: Some(e.id.clone()), ..entry("Hike", "Swam in the lake", &["outdoors"]) })
            .unwrap();
        assert_eq!(e2.tags, vec!["outdoors"]);
        assert!(s.search("ridge", 10).unwrap().is_empty());
        assert_eq!(s.search("lake", 10).unwrap().len(), 1);
        assert_eq!(s.list_tags().unwrap().len(), 1, "unused tags are dropped");

        let only = s.list_entries(&EntryQuery { tag: Some("outdoors".into()), ..Default::default() }).unwrap();
        assert_eq!(only.len(), 1);

        s.trash("entry", &e.id).unwrap();
        assert!(s.list_entries(&EntryQuery::default()).unwrap().is_empty());
        assert!(s.search("lake", 10).unwrap().is_empty(), "trashed items are not searchable");
        assert_eq!(s.list_trash().unwrap().len(), 1);
        s.restore("entry", &e.id).unwrap();
        assert_eq!(s.search("lake", 10).unwrap().len(), 1);
        s.purge("entry", &e.id).unwrap();
        assert!(s.get_entry(&e.id).is_err());
    }

    #[test]
    fn client_generated_id_creates_entry() {
        let (_d, s) = store();
        let id = "11111111-2222-3333-4444-555555555555".to_string();
        let e = s.save_entry(EntryInput { id: Some(id.clone()), ..entry("", "draft", &[]) }).unwrap();
        assert_eq!(e.id, id);
    }

    #[test]
    fn notes_hashtags_and_folders() {
        let (_d, s) = store();
        let f = s.save_folder(FolderInput { id: None, parent_id: None, name: "Work".into() }).unwrap();
        let sub = s.save_folder(FolderInput { id: None, parent_id: Some(f.clone()), name: "Ideas".into() }).unwrap();
        // Cycle guard.
        assert!(s.save_folder(FolderInput { id: Some(f.clone()), parent_id: Some(sub.clone()), name: "Work".into() }).is_err());

        let n = s
            .save_note(NoteInput {
                id: None,
                folder_id: Some(sub.clone()),
                title: "Plan".into(),
                body: "# Heading\nShip it #release and #q4-goals, not `#code` or #1".into(),
                pinned: false,
            })
            .unwrap();
        assert_eq!(n.tags, vec!["q4-goals", "release"]);
        assert_eq!(s.list_notes(Some(sub.clone()), None).unwrap().len(), 1);
        assert_eq!(s.list_notes(None, Some("release".into())).unwrap().len(), 1);

        s.delete_folder(&sub).unwrap();
        let moved = s.get_note(&n.id).unwrap();
        assert_eq!(moved.folder_id, Some(f), "notes move up to the parent folder");
    }

    #[test]
    fn media_roundtrip_with_thumbnail() {
        let (_d, s) = store();
        let e = s.save_entry(entry("Pic", "", &[])).unwrap();
        let mut png = Vec::new();
        image::RgbImage::from_pixel(1200, 800, image::Rgb([200, 100, 50]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let m = s
            .add_media(NewMedia { item_kind: "entry", item_id: &e.id, filename: "a.png", mime: "image/png", bytes: &png })
            .unwrap();
        assert_eq!(m.kind, "image");
        let thumb = m.thumb_path.clone().unwrap();
        let (w, h) = image::image_dimensions(&thumb).unwrap();
        assert!(w <= 480 && h <= 480);
        let list = s.list_entries(&EntryQuery::default()).unwrap();
        assert_eq!(list[0].media_count, 1);
        assert_eq!(list[0].cover_thumb.as_deref(), Some(thumb.as_str()));

        s.purge("entry", &e.id).unwrap();
        assert!(!Path::new(&m.path).exists());
        assert!(!Path::new(&thumb).exists());
    }

    #[test]
    fn reminders() {
        let (_d, s) = store();
        let r = s
            .save_reminder(Reminder {
                id: None,
                title: "Journal".into(),
                body: String::new(),
                next_at: "2026-11-21T03:30:00.000Z".into(),
                repeat: "daily".into(),
                enabled: true,
                last_fired_at: None,
            })
            .unwrap();
        assert_eq!(s.list_reminders().unwrap().len(), 1);
        assert!(s.save_reminder(Reminder { repeat: "hourly".into(), ..r.clone() }).is_err());
        s.delete_reminder(r.id.as_deref().unwrap()).unwrap();
        assert!(s.list_reminders().unwrap().is_empty());
    }

    #[test]
    fn excerpt_strips_markdown() {
        assert_eq!(excerpt("## Plan\n- [ ] Order **seeds**\n> quote `x`", 100), "Plan Order seeds quote x");
    }

    #[test]
    fn fts_query_is_safe() {
        assert_eq!(fts_query("  "), None);
        assert_eq!(fts_query("foo \"bar"), Some("\"foo\" \"bar\"*".into()));
        assert_eq!(fts_query("NOT OR AND(").unwrap(), "\"NOT\" \"OR\" \"AND(\"*");
    }
}
