use rusqlite::{params, Connection, Result};
use sqlite_vec::sqlite3_vec_init;

pub struct Database {
    conn: Option<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Self {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite3_vec_init as *const (),
            )));
        }
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "PRAGMA busy_timeout=5000; PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;",
        )
        .unwrap_or_else(|_| println!("Pragma journal execution returned error"));
        Self { conn: Some(conn) }
    }

    pub fn init_schemas(&self) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_queue_receipts (
                source TEXT NOT NULL, source_id INTEGER NOT NULL,
                PRIMARY KEY(source, source_id)
            );
            CREATE TABLE IF NOT EXISTS sync_receipts (
                id TEXT PRIMARY KEY, queue_id INTEGER UNIQUE, note_id INTEGER UNIQUE
            );
            CREATE TABLE IF NOT EXISTS sync_operations (
                id TEXT PRIMARY KEY, outcome TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS categories (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notes (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
                text        TEXT NOT NULL,
                date        TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notes_queue (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                text TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS note_embeddings USING vec0(
                note_id INTEGER PRIMARY KEY,
                embedding FLOAT[768]
            );
        ",
        )?;
        // Existing notes start read. A trigger tracks all later inserts, including phone sync.
        conn.execute_batch(
            "BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS note_activity (
                note_id INTEGER PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                seen INTEGER NOT NULL DEFAULT 0 CHECK(seen IN (0,1))
            );
            INSERT OR IGNORE INTO note_activity(note_id,seen) SELECT id,1 FROM notes;
            CREATE TRIGGER IF NOT EXISTS track_new_notes AFTER INSERT ON notes BEGIN
                INSERT INTO note_activity(note_id,seen) VALUES (NEW.id,0);
            END;
            COMMIT;",
        )?;
        // FLOAT[768] fits for nomic-embed-text-v2-moe
        Ok(())
    }

    pub fn category_stats(&self) -> Result<Vec<(i64, String, i64, i64)>> {
        let mut stmt=self.connection().prepare("SELECT c.id,c.name,COUNT(n.id),COALESCE(SUM(CASE WHEN a.seen=0 THEN 1 ELSE 0 END),0) FROM categories c LEFT JOIN notes n ON n.category_id=c.id LEFT JOIN note_activity a ON a.note_id=n.id GROUP BY c.id ORDER BY c.name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect()
    }

    pub fn mark_category_seen(&self, category_id: i64) -> Result<()> {
        self.connection().execute("UPDATE note_activity SET seen=1 WHERE note_id IN (SELECT id FROM notes WHERE category_id=?1)",[category_id])?;
        Ok(())
    }

    pub fn latest_note(&self) -> Result<Option<(i64, String, String)>> {
        use rusqlite::OptionalExtension;
        self.connection().query_row("SELECT n.id,c.name,n.text FROM notes n JOIN categories c ON c.id=n.category_id ORDER BY n.id DESC LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()
    }

    pub fn category_for_note(&self, note_id: i64) -> Result<String> {
        self.connection().query_row(
            "SELECT c.name FROM notes n JOIN categories c ON c.id=n.category_id WHERE n.id=?1",
            [note_id],
            |r| r.get(0),
        )
    }

    pub fn connection(&self) -> &Connection {
        self.conn.as_ref().unwrap()
    }

    pub fn finish_queue(
        &self,
        queue_id: i64,
        category: &str,
        text: &str,
        date: &str,
        embeddings: Vec<f32>,
    ) -> Result<()> {
        let tx = self.connection().unchecked_transaction()?;
        // The desktop may have removed this queue item while AI was processing it.
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes_queue WHERE id=?1 AND text=?2)",
            params![queue_id,text],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(());
        }
        tx.execute("INSERT INTO categories(name) SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM categories WHERE lower(name)=lower(?1))", [category])?;
        let category_id: i64 = tx.query_row(
            "SELECT id FROM categories WHERE lower(name)=lower(?1) LIMIT 1",
            [category],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO notes(category_id,text,date) VALUES (?1,?2,?3)",
            params![category_id, text, date],
        )?;
        let id = tx.last_insert_rowid();
        let bytes: Vec<u8> = embeddings.iter().flat_map(|f| f.to_le_bytes()).collect();
        tx.execute(
            "INSERT INTO note_embeddings(note_id,embedding) VALUES (?1,?2)",
            params![id, bytes],
        )?;
        tx.execute(
            "UPDATE sync_receipts SET note_id=?1,queue_id=NULL WHERE queue_id=?2",
            params![id, queue_id],
        )?;
        tx.execute("DELETE FROM notes_queue WHERE id=?1", [queue_id])?;
        tx.commit()
    }

    pub fn insert_embeddings_if_text(&self, note_id: i64, text: &str, embeddings: Vec<f32>) -> Result<()> {
        let tx=self.connection().unchecked_transaction()?;
        let current:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND text=?2)",params![note_id,text],|r|r.get(0))?;
        if current {
            let bytes:Vec<u8>=embeddings.iter().flat_map(|f|f.to_le_bytes()).collect();
            tx.execute("DELETE FROM note_embeddings WHERE note_id=?1",[note_id])?;
            tx.execute("INSERT INTO note_embeddings(note_id,embedding) VALUES (?1,?2)",params![note_id,bytes])?;
        }
        tx.commit()
    }

    pub fn search_by_embeddings(
        &self,
        embeddings: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<(i64, String, String, String)>> {
        let conn = self.conn.as_ref().unwrap();
        let bytes: Vec<u8> = embeddings.iter().flat_map(|f| f.to_le_bytes()).collect();

        let sql = format!(
            "
            SELECT n.id,n.text,n.date,c.name
            FROM notes n
            JOIN categories c ON c.id=n.category_id
            JOIN (
                SELECT note_id
                FROM note_embeddings
                WHERE embedding MATCH ?1
                ORDER BY distance
                LIMIT {limit}
            ) e ON n.id = e.note_id
        "
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![bytes], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect()
    }

    pub fn insert_to_queue(&self, text: &str) -> Result<i64> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("INSERT INTO notes_queue (text) VALUES (?1)", params![text])?;
        Ok(conn.last_insert_rowid())
    }

    #[cfg(test)]
    pub fn insert_category(&self, name: &str) -> Result<i64> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("INSERT INTO categories (name) VALUES (?1)", params![name])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn insert_note(&self, category_id: i64, text: &str, date: &str) -> Result<i64> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute(
            "INSERT INTO notes (category_id, text, date) VALUES (?1, ?2, ?3)",
            params![category_id, text, date],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_categories(&self) -> Result<Vec<(i64, String)>> {
        let conn = self.conn.as_ref().unwrap();
        let mut stmt = conn.prepare("SELECT id, name FROM categories ORDER BY name")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn get_notes(&self, category_id: i64) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn.as_ref().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, text, date FROM notes WHERE category_id = ?1 ORDER BY id DESC")?;
        let rows = stmt.query_map(params![category_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect()
    }

    pub fn get_queue_items(&self) -> Result<Vec<(i64, String)>> {
        let conn = self.conn.as_ref().unwrap();
        let mut stmt = conn.prepare("SELECT id, text FROM notes_queue ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn delete_queue_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("DELETE FROM notes_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        let tx = self.connection().unchecked_transaction()?;
        tx.execute("DELETE FROM note_embeddings WHERE note_id=?1", [id])?;
        tx.execute("DELETE FROM notes WHERE id=?1", [id])?;
        tx.commit()
    }

    /// Renames a category by id
    pub fn rename_category(&self, id: i64, new_name: &str) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute(
            "UPDATE categories SET name = ?1 WHERE id = ?2",
            params![new_name, id],
        )?;
        Ok(())
    }

    /// Deletes category, notes, and their embeddings
    pub fn delete_category(&self, id: i64) -> Result<()> {
        let tx = self.connection().unchecked_transaction()?;
        tx.execute("DELETE FROM note_embeddings WHERE note_id IN (SELECT id FROM notes WHERE category_id=?1)",[id])?;
        tx.execute("DELETE FROM categories WHERE id=?1", [id])?;
        tx.commit()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unread_migration_survives_restart_and_counts_only_existing_notes() {
        let path =
            std::env::temp_dir().join(format!("notes-ai-unread-{}.db", uuid::Uuid::new_v4()));
        {
            let db = Database::new(path.to_str().unwrap());
            db.init_schemas().unwrap();
            let category = db.insert_category("Ideas").unwrap();
            let first = db.insert_note(category, "first", "today").unwrap();
            db.insert_note(category, "second", "today").unwrap();
            assert_eq!(db.category_stats().unwrap()[0].3, 2);
            // Reopening/migrating cannot silently mark new notes read.
            db.init_schemas().unwrap();
            assert_eq!(db.category_stats().unwrap()[0].3, 2);
            db.delete_note(first).unwrap();
            assert_eq!(db.category_stats().unwrap()[0].3, 1);
        }
        {
            let db = Database::new(path.to_str().unwrap());
            db.init_schemas().unwrap();
            let category = db.get_categories().unwrap()[0].0;
            assert_eq!(db.category_stats().unwrap()[0].3, 1);
            db.mark_category_seen(category).unwrap();
            assert_eq!(db.category_stats().unwrap()[0].3, 0);
            db.insert_note(category, "third", "today").unwrap();
            assert_eq!(db.category_stats().unwrap()[0].3, 1);
            db.delete_category(category).unwrap();
            assert!(db.category_stats().unwrap().is_empty());
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn queue_classification_is_atomic_and_keeps_phone_identity() {
        let path = std::env::temp_dir().join(format!("notes-ai-{}.db", uuid::Uuid::new_v4()));
        {
            let db = Database::new(path.to_str().unwrap());
            db.init_schemas().unwrap();
            let queue = db.insert_to_queue("Offline phone note").unwrap();
            db.connection()
                .execute(
                    "INSERT INTO sync_receipts(id,queue_id) VALUES ('phone-identity',?1)",
                    [queue],
                )
                .unwrap();
            // Invalid vector must roll back the note, category, receipt and queue transition.
            assert!(db
                .finish_queue(queue, "Ideas", "Offline phone note", "today", vec![0.0; 2])
                .is_err());
            assert_eq!(db.get_queue_items().unwrap().len(), 1);
            assert!(db.get_categories().unwrap().is_empty());
            db.finish_queue(
                queue,
                "Ideas",
                "Offline phone note",
                "today",
                vec![0.0; 768],
            )
            .unwrap();
            assert!(db.get_queue_items().unwrap().is_empty());
            let id: i64 = db
                .connection()
                .query_row(
                    "SELECT note_id FROM sync_receipts WHERE id='phone-identity'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                db.get_notes(db.get_categories().unwrap()[0].0).unwrap()[0].0,
                id
            );
            // A stale worker must not create a second note.
            db.finish_queue(
                queue,
                "Ideas",
                "Offline phone note",
                "today",
                vec![0.0; 768],
            )
            .unwrap();
            assert_eq!(
                db.get_notes(db.get_categories().unwrap()[0].0)
                    .unwrap()
                    .len(),
                1
            );
            // Receipts survive deletion so delayed retries cannot resurrect a phone note.
            db.delete_note(id).unwrap();
            let receipts: i64 = db
                .connection()
                .query_row(
                    "SELECT count(*) FROM sync_receipts WHERE id='phone-identity'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(receipts, 1);
        }
        std::fs::remove_file(path).unwrap();
    }
}
