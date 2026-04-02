use rusqlite::{Connection, Result, params};
use sqlite_vec::sqlite3_vec_init;

pub struct Database {
    conn: Option<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Self {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        unsafe { rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ()))); }
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").unwrap_or_else(|_| println!("Pragma journal execution returned error"));
        Self { conn: Some(conn) }
    }

    pub fn init_schemas(&self) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute_batch("
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
        ")?;
        // FLOAT[768] fits for nomic-embed-text-v2-moe
        Ok(())
    }

    pub fn insert_embeddings(&self, note_id: i64, embeddings: Vec<f32>) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        // Сериализуем Vec<f32> в байты (little-endian)
        let bytes: Vec<u8> = embeddings.iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        conn.execute(
            "INSERT INTO note_embeddings (note_id, embedding) VALUES (?1, ?2)",
            params![note_id, bytes],
        )?;
        Ok(())
    }

    pub fn search_by_embeddings(&self, embeddings: Vec<f32>, limit: u32) -> Vec<String> {
        let conn = self.conn.as_ref().unwrap();
        let bytes: Vec<u8> = embeddings.iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        let sql = format!("
            SELECT n.text
            FROM notes n
            JOIN (
                SELECT note_id
                FROM note_embeddings
                WHERE embedding MATCH ?1
                ORDER BY distance
                LIMIT {limit}
            ) e ON n.id = e.note_id
        ");
        let mut stmt = conn.prepare(&sql).unwrap();

        stmt.query_map(params![bytes], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn delete_embeddings(&self, note_id: i64) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("DELETE FROM note_embeddings WHERE note_id = ?1", params![note_id])?;
        Ok(())
    }

    pub fn insert_to_queue(&self, text: &str) -> Result<i64> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("INSERT INTO notes_queue (text) VALUES (?1)", params![text])?;
        Ok(conn.last_insert_rowid())
    }

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
        let mut stmt = conn.prepare(
            "SELECT id, text, date FROM notes WHERE category_id = ?1 ORDER BY id DESC"
        )?;
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
        let conn = self.conn.as_ref().unwrap();
        conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }
    
    /// Renames a category by id
    pub fn rename_category(&self, id: i64, new_name: &str) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute("UPDATE categories SET name = ?1 WHERE id = ?2", params![new_name, id])?;
        Ok(())
    }

    /// Deletes category, notes, and their embeddings
    pub fn delete_category(&self, id: i64) -> Result<()> {
        let conn = self.conn.as_ref().unwrap();
        conn.execute(
            "DELETE FROM note_embeddings WHERE note_id IN (SELECT id FROM notes WHERE category_id = ?1)",
            params![id]
        )?;
        conn.execute("DELETE FROM categories WHERE id = ?1", params![id])?;
        Ok(())
    }
}