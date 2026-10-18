//! Portable note archives and additive legacy SQLite import. No settings or secrets are exported.
use crate::db::Database;
use rusqlite::{params, Connection, DatabaseName, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    io::Write,
    path::{Path, PathBuf},
};
pub type TransferResult<T> = Result<T, Box<dyn Error>>;
const MAX_BYTES: u64 = 100_000_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub category_id: i64,
    pub text: String,
    pub date: String,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedNote {
    pub id: i64,
    pub text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub embedding_model: Option<String>,
    pub categories: Vec<Category>,
    pub notes: Vec<Note>,
    pub queue: Vec<QueuedNote>,
}
#[derive(Debug, Default)]
pub struct ImportReport {
    pub added_notes: usize,
    pub skipped_notes: usize,
    pub added_categories: usize,
    pub added_queue: usize,
    pub indexed: usize,
    pub awaiting_index: usize,
}
impl std::fmt::Display for ImportReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"Imported {} notes, {} collections, {} queued notes. {} existing notes skipped. Search: {} vectors restored, {} notes awaiting indexing.",self.added_notes,self.added_categories,self.added_queue,self.skipped_notes,self.indexed,self.awaiting_index)
    }
}

impl Archive {
    pub fn validate(&self) -> TransferResult<()> {
        if self.format != "notes-ai" || self.version != 1 {
            return Err("Unsupported archive format/version. Expected Notes AI version 1.".into());
        }
        if self.notes.len() > 200_000
            || self.queue.len() > 200_000
            || self.categories.len() > 20_000
        {
            return Err("Archive is too large.".into());
        }
        let mut categories = HashSet::new();
        for c in &self.categories {
            if !categories.insert(c.id) || c.name.trim().is_empty() || c.name.len() > 4000 {
                return Err("Invalid or duplicate collection in archive.".into());
            }
        }
        let mut ids = HashSet::new();
        for n in &self.notes {
            if !ids.insert(n.id)
                || !categories.contains(&n.category_id)
                || n.text.len() > 2_000_000
                || n.date.len() > 1000
            {
                return Err("Invalid note or missing collection in archive.".into());
            }
            if let Some(vector) = &n.embedding {
                if vector.len() != 768 || vector.iter().any(|v| !v.is_finite()) {
                    return Err("An archive vector is invalid; expected 768 finite values.".into());
                }
            }
        }
        ids.clear();
        for q in &self.queue {
            if !ids.insert(q.id) || q.text.len() > 2_000_000 {
                return Err("Invalid queued note in archive.".into());
            }
        }
        Ok(())
    }
    pub fn summary(&self, current_model: &str) -> String {
        let index=match self.embedding_model.as_deref(){Some(model) if model!=current_model=>"The embedding model differs. Notes will be reindexed with your current model.",None=>"Legacy SQLite backup: saved search vectors will be retained. Keep the original embedding model.",_=>"Saved search vectors will be retained."};
        format!("{} notes · {} collections · {} queued\n\nExisting notes will be kept. Matching notes are skipped, and a safety backup is created before importing.\n\n{}",self.notes.len(),self.categories.len(),self.queue.len(),index)
    }
}

fn snapshot(conn: &Connection, model: Option<String>) -> TransferResult<Archive> {
    let tx = conn.unchecked_transaction()?;
    let mut stmt = tx.prepare("SELECT id,name FROM categories ORDER BY id")?;
    let categories = stmt
        .query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let has_vectors: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='note_embeddings' AND type='table')",
        [],
        |r| r.get(0),
    )?;
    let sql = if has_vectors {
        "SELECT n.id,n.category_id,n.text,n.date,e.embedding FROM notes n LEFT JOIN note_embeddings e ON e.note_id=n.id ORDER BY n.id"
    } else {
        "SELECT id,category_id,text,date,NULL FROM notes ORDER BY id"
    };
    let mut stmt = tx.prepare(sql)?;
    let notes = stmt
        .query_map([], |r| {
            let blob: Option<Vec<u8>> = r.get(4)?;
            let embedding = blob.map(|bytes| {
                bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect()
            });
            Ok(Note {
                id: r.get(0)?,
                category_id: r.get(1)?,
                text: r.get(2)?,
                date: r.get(3)?,
                embedding,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = tx.prepare("SELECT id,text FROM notes_queue ORDER BY id")?;
    let queue = stmt
        .query_map([], |r| {
            Ok(QueuedNote {
                id: r.get(0)?,
                text: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let archive = Archive {
        format: "notes-ai".into(),
        version: 1,
        embedding_model: model,
        categories,
        notes,
        queue,
    };
    archive.validate()?;
    Ok(archive)
}

pub fn export_archive(db: &Database, model: &str) -> TransferResult<Archive> {
    snapshot(db.connection(), Some(model.into()))
}

pub fn read_archive(path: &Path) -> TransferResult<Archive> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_BYTES {
        return Err("Backup exceeds the 100 MB import limit.".into());
    }
    let mut file = std::fs::File::open(path)?;
    let mut signature = [0u8; 16];
    use std::io::Read;
    let count = file.read(&mut signature)?;
    let archive = if count == 16 && &signature == b"SQLite format 3\0" {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
        // Immutable read-only URI prevents even WAL/SHM sidecars being created next to the source backup.
        let mut uri = reqwest::Url::from_file_path(path.canonicalize()?)
            .map_err(|_| "Invalid SQLite backup path")?;
        uri.query_pairs_mut().append_pair("immutable", "1");
        let conn = Connection::open_with_flags(
            uri.as_str(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        for table in ["categories", "notes", "notes_queue"] {
            let kind: Option<String> = conn
                .query_row(
                    "SELECT type FROM sqlite_master WHERE name=?1",
                    [table],
                    |r| r.get(0),
                )
                .optional()?;
            if kind.as_deref() != Some("table") {
                return Err(format!("This backup is missing the {table} table.").into());
            }
        }
        let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err(format!("SQLite backup integrity check failed: {check}").into());
        }
        snapshot(&conn, None)?
    } else {
        serde_json::from_reader(std::fs::File::open(path)?)?
    };
    archive.validate()?;
    Ok(archive)
}

pub fn import_archive(
    db: &Database,
    archive: &Archive,
    current_model: &str,
) -> TransferResult<ImportReport> {
    archive.validate()?;
    let source = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            archive.categories.clone(),
            archive.notes.clone(),
            archive.queue.clone()
        ))?)
    );
    let use_vectors = archive
        .embedding_model
        .as_deref()
        .map(|m| m == current_model)
        .unwrap_or(true);
    let tx = db.connection().unchecked_transaction()?;
    let mut report = ImportReport::default();
    let mut names = HashMap::<String, i64>::new();
    {
        let mut stmt = tx.prepare("SELECT id,name FROM categories ORDER BY id")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, name) = row?;
            names.entry(name.trim().to_lowercase()).or_insert(id);
        }
    }
    let mut categories = HashMap::new();
    for c in &archive.categories {
        let normalized = c.name.trim().to_lowercase();
        let id = if let Some(id) = names.get(&normalized) {
            *id
        } else {
            tx.execute("INSERT INTO categories(name) VALUES (?1)", [&c.name])?;
            let id = tx.last_insert_rowid();
            names.insert(normalized, id);
            report.added_categories += 1;
            id
        };
        categories.insert(c.id, id);
    }
    // Match occurrences, not just a set: intentional identical notes within a backup survive.
    let mut occurrences = HashMap::<(i64, String, String), usize>::new();
    for n in &archive.notes {
        let category_id = categories[&n.category_id];
        let occurrence = occurrences
            .entry((category_id, n.text.clone(), n.date.clone()))
            .or_default();
        let matches = {
            let mut stmt = tx.prepare(
                "SELECT id FROM notes WHERE category_id=?1 AND text=?2 AND date=?3 ORDER BY id",
            )?;
            let rows = stmt
                .query_map(params![category_id, n.text, n.date], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let id = if let Some(id) = matches.get(*occurrence) {
            report.skipped_notes += 1;
            *id
        } else {
            tx.execute(
                "INSERT INTO notes(category_id,text,date) VALUES (?1,?2,?3)",
                params![category_id, n.text, n.date],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("UPDATE note_activity SET seen=1 WHERE note_id=?1", [id])?;
            report.added_notes += 1;
            id
        };
        *occurrence += 1;
        let indexed: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM note_embeddings WHERE note_id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if !indexed {
            if let Some(vector) = n.embedding.as_ref().filter(|_| use_vectors) {
                let bytes = vector
                    .iter()
                    .flat_map(|f| f.to_le_bytes())
                    .collect::<Vec<_>>();
                tx.execute(
                    "INSERT INTO note_embeddings(note_id,embedding) VALUES (?1,?2)",
                    params![id, bytes],
                )?;
                report.indexed += 1;
            } else {
                report.awaiting_index += 1;
            }
        }
    }
    let mut queue_occurrences = HashMap::<String, usize>::new();
    for q in &archive.queue {
        let already: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_queue_receipts WHERE source=?1 AND source_id=?2)",
            params![source, q.id],
            |r| r.get(0),
        )?;
        let occurrence = queue_occurrences.entry(q.text.clone()).or_default();
        *occurrence += 1;
        if already {
            continue;
        }
        let existing: i64 = tx.query_row(
            "SELECT count(*) FROM notes_queue WHERE text=?1",
            [&q.text],
            |r| r.get(0),
        )?;
        if existing < *occurrence as i64 {
            tx.execute("INSERT INTO notes_queue(text) VALUES (?1)", [&q.text])?;
            report.added_queue += 1;
        }
        tx.execute(
            "INSERT INTO import_queue_receipts(source,source_id) VALUES (?1,?2)",
            params![source, q.id],
        )?;
    }
    tx.commit()?;
    Ok(report)
}

fn safe_output(destination: &Path, db_path: &Path) -> TransferResult<()> {
    let resolved = if destination.exists() {
        destination.canonicalize()?
    } else {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        parent
            .canonicalize()?
            .join(destination.file_name().ok_or("Choose a file name")?)
    };
    let db = db_path.canonicalize()?;
    if resolved == db
        || resolved == PathBuf::from(format!("{}-wal", db.display()))
        || resolved == PathBuf::from(format!("{}-shm", db.display()))
    {
        return Err(
            "Choose a different file; the active database cannot be an export destination.".into(),
        );
    }
    Ok(())
}
fn atomic_file(path: &Path) -> TransferResult<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(tempfile::NamedTempFile::new_in(parent)?)
}

pub fn write_json(db: &Database, model: &str, path: &Path, db_path: &Path) -> TransferResult<()> {
    safe_output(path, db_path)?;
    let archive = export_archive(db, model)?;
    let mut file = atomic_file(path)?;
    serde_json::to_writer_pretty(&mut file, &archive)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
pub fn write_sqlite(db: &Database, path: &Path, db_path: &Path) -> TransferResult<()> {
    safe_output(path, db_path)?;
    // SQLite must own its Windows file handle; close tempfile's handle before backup.
    let temporary = atomic_file(path)?.into_temp_path();
    db.connection()
        .backup(DatabaseName::Main, &temporary, None)?;
    {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&temporary)?;
        file.sync_all()?;
    }
    temporary.persist(path)?;
    Ok(())
}
pub fn write_markdown(
    db: &Database,
    model: &str,
    path: &Path,
    db_path: &Path,
) -> TransferResult<()> {
    safe_output(path, db_path)?;
    let archive = export_archive(db, model)?;
    let mut file = atomic_file(path)?;
    writeln!(file, "# Notes AI\n")?;
    for category in archive.categories {
        writeln!(file, "## {}\n", category.name.replace(['\r', '\n'], " "))?;
        for note in archive
            .notes
            .iter()
            .filter(|n| n.category_id == category.id)
        {
            writeln!(
                file,
                "### {}\n\n{}\n\n---\n",
                note.date.replace(['\r', '\n'], " "),
                note.text
            )?;
        }
    }
    if !archive.queue.is_empty() {
        writeln!(file, "## Awaiting AI\n")?;
        for note in archive.queue {
            writeln!(file, "{}\n\n---\n", note.text)?;
        }
    }
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
pub fn safety_backup(db: &Database, db_path: &Path) -> TransferResult<PathBuf> {
    let folder = db_path.parent().unwrap_or(Path::new(".")).join("backups");
    std::fs::create_dir_all(&folder)?;
    let path = folder.join(format!(
        "before-import-{}-{}.db",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4()
    ));
    write_sqlite(db, &path, db_path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive() -> Archive {
        Archive {
            format: "notes-ai".into(),
            version: 1,
            embedding_model: Some("model".into()),
            categories: vec![Category {
                id: 77,
                name: "Ideas".into(),
            }],
            notes: vec![Note {
                id: 9,
                category_id: 77,
                text: "Keep the full text\nwith a second line".into(),
                date: "18 October 2026".into(),
                embedding: Some(vec![0.25; 768]),
            }],
            queue: vec![QueuedNote {
                id: 123,
                text: "Waiting thought".into(),
            }],
        }
    }
    fn db(path: &Path) -> Database {
        let db = Database::new(path.to_str().unwrap());
        db.init_schemas().unwrap();
        db
    }
    #[test]
    fn merge_preserves_existing_and_intentional_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(&dir.path().join("notes.db"));
        let category = db.insert_category("Ideas").unwrap();
        db.insert_note(category, "unrelated existing note", "yesterday")
            .unwrap();
        let mut archive = archive();
        let mut duplicate = archive.notes[0].clone();
        duplicate.id = 10;
        archive.notes.push(duplicate);
        let first = import_archive(&db, &archive, "model").unwrap();
        assert_eq!(first.added_notes, 2);
        let repeated = import_archive(&db, &archive, "model").unwrap();
        assert_eq!(repeated.added_notes, 0);
        assert_eq!(repeated.skipped_notes, 2);
        assert_eq!(db.get_notes(category).unwrap().len(), 3);
        assert_eq!(db.category_stats().unwrap()[0].3, 1); // Only the original new note is unread.
    }
    #[test]
    fn queue_receipt_survives_processing_and_prevents_reimport() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(&dir.path().join("notes.db"));
        let archive = archive();
        import_archive(&db, &archive, "model").unwrap();
        let queued = db.get_queue_items().unwrap();
        assert_eq!(queued.len(), 1);
        db.finish_queue(queued[0].0, "Ideas", &queued[0].1, "today", vec![0.0; 768])
            .unwrap();
        let report = import_archive(&db, &archive, "model").unwrap();
        assert_eq!(report.added_queue, 0);
        assert!(db.get_queue_items().unwrap().is_empty());
    }
    #[test]
    fn json_and_sqlite_roundtrips_keep_text_dates_vectors_and_queue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.db");
        let db = db(&path);
        import_archive(&db, &archive(), "model").unwrap();
        let json = dir.path().join("export.json");
        write_json(&db, "model", &json, &path).unwrap();
        let loaded = read_archive(&json).unwrap();
        assert_eq!(loaded.notes[0].text, archive().notes[0].text);
        assert_eq!(loaded.notes[0].date, archive().notes[0].date);
        assert_eq!(loaded.notes[0].embedding, archive().notes[0].embedding);
        assert_eq!(loaded.queue.len(), 1);
        let sqlite = dir.path().join("export.db");
        write_sqlite(&db, &sqlite, &path).unwrap();
        let before = std::fs::read(&sqlite).unwrap();
        let loaded = read_archive(&sqlite).unwrap();
        assert_eq!(loaded.notes.len(), 1);
        assert_eq!(loaded.queue.len(), 1);
        assert_eq!(loaded.notes[0].embedding, archive().notes[0].embedding);
        assert_eq!(std::fs::read(&sqlite).unwrap(), before);
        assert!(!PathBuf::from(format!("{}-wal", sqlite.display())).exists());
        assert!(write_json(&db, "model", &path, &path).is_err());
    }
    #[test]
    fn invalid_archive_cannot_partially_import() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(&dir.path().join("notes.db"));
        let mut archive = archive();
        archive.notes[0].category_id = 999;
        assert!(import_archive(&db, &archive, "model").is_err());
        assert!(db.get_categories().unwrap().is_empty());
        archive.notes[0].category_id = 77;
        archive.notes[0].embedding = Some(vec![f32::NAN; 768]);
        assert!(import_archive(&db, &archive, "model").is_err());
        assert!(db.get_categories().unwrap().is_empty());
        assert!(db.get_queue_items().unwrap().is_empty());
    }
    #[test]
    fn differing_embedding_model_keeps_notes_but_requests_reindexing() {
        let dir = tempfile::tempdir().unwrap();
        let db = db(&dir.path().join("notes.db"));
        let report = import_archive(&db, &archive(), "different-model").unwrap();
        assert_eq!(report.added_notes, 1);
        assert_eq!(report.indexed, 0);
        assert_eq!(report.awaiting_index, 1);
        let count: i64 = db
            .connection()
            .query_row("SELECT count(*) FROM note_embeddings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
    #[test]
    fn safety_backup_contains_preimport_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.db");
        let db = db(&path);
        let category = db.insert_category("Original").unwrap();
        db.insert_note(category, "existing", "yesterday").unwrap();
        let copy = safety_backup(&db, &path).unwrap();
        import_archive(&db, &archive(), "model").unwrap();
        let before = read_archive(&copy).unwrap();
        assert_eq!(before.notes.len(), 1);
        assert_eq!(before.notes[0].text, "existing");
        let after = export_archive(&db, "model").unwrap();
        assert_eq!(after.notes.len(), 2);
    }
}
