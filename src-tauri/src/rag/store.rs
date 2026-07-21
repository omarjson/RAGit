use std::path::PathBuf;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::Serialize;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct Store {
    pool: DbPool,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Store { pool: self.pool.clone() }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct RetrievedChunk {
    pub id: i64,
    pub library_id: String,
    pub file_name: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f32,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndexStatus {
    Pending,
    Indexing,
    Paused,
    Done,
    Error,
    Canceled,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub library_id: String,
    pub path: String,
    pub file_name: String,
    pub content_hash: String,
    pub status: String,
    pub level: i64,
    pub chunks: i64,
    pub error: Option<String>,
}

fn db_path() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("ragit");
    std::fs::create_dir_all(&dir).ok();
    dir.push("rag.db");
    dir
}

impl Store {
    pub fn new() -> Result<Store, String> {
        let manager = SqliteConnectionManager::file(db_path());
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| format!("failed to create DB pool: {e}"))?;

        let store = Store { pool };
        store.init_schema().map_err(|e| format!("init schema: {e}"))?;
        Ok(store)
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, String> {
        self.pool.get().map_err(|e| format!("DB pool error: {e}"))
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS libraries (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                index_cursor TEXT
            );
            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                library_id TEXT NOT NULL,
                path TEXT NOT NULL,
                file_name TEXT NOT NULL,
                content_hash TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                indexed_level INTEGER DEFAULT 0,
                chunks INTEGER DEFAULT 0,
                error TEXT,
                started_at INTEGER,
                finished_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY,
                library_id TEXT NOT NULL,
                file_id INTEGER NOT NULL,
                file_name TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                level INTEGER DEFAULT 1,
                content TEXT NOT NULL,
                embedding BLOB
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_lib ON chunks(library_id);
            CREATE INDEX IF NOT EXISTS idx_files_lib ON files(library_id);",
        )
        .map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'viewer'
            );
            CREATE TABLE IF NOT EXISTS library_members (
                library_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'viewer',
                PRIMARY KEY (library_id, user_id)
            );
            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_library(&self, id: &str, name: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR IGNORE INTO libraries (id, name) VALUES (?1, ?2)",
            params![id, name],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_index_cursor(&self, library_id: &str, cursor: Option<&str>) -> Result<(), String> {
        let conn = self.conn()?;
        if let Some(c) = cursor {
            conn.execute(
                "UPDATE libraries SET index_cursor = ?1 WHERE id = ?2",
                params![c, library_id],
            ).map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE libraries SET index_cursor = NULL WHERE id = ?1",
                params![library_id],
            ).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn get_index_cursor(&self, library_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT index_cursor FROM libraries WHERE id = ?1",
            params![library_id],
            |r| r.get(0),
        ).map_err(|e| e.to_string())
    }

    /// Reset any files stuck in "indexing" status back to "pending" (crash recovery).
    pub fn reset_stuck_files(&self) -> Result<usize, String> {
        let conn = self.conn()?;
        let count = conn.execute(
            "UPDATE files SET status = 'pending', started_at = NULL WHERE status = 'indexing'",
            [],
        ).map_err(|e| e.to_string())?;
        Ok(count)
    }

    pub fn add_file(
        &self,
        library_id: &str,
        path: &str,
        file_name: &str,
        content_hash: &str,
    ) -> Result<i64, String> {
        let conn = self.conn()?;
        conn.query_row(
            "INSERT INTO files (library_id, path, file_name, content_hash)
             VALUES (?1,?2,?3,?4) RETURNING id",
            params![library_id, path, file_name, content_hash],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    }

    pub fn upsert_file(
        &self,
        library_id: &str,
        path: &str,
        file_name: &str,
        content_hash: &str,
    ) -> Result<i64, String> {
        let conn = self.conn()?;
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM files WHERE library_id = ?1 AND path = ?2",
                params![library_id, path],
                |r| r.get(0),
            )
            .ok();
        if let Some(id) = existing {
            conn.execute(
                "UPDATE files SET status='pending', indexed_level=0, chunks=0, error=NULL,
                    started_at=NULL, finished_at=NULL WHERE id = ?1",
                params![id],
            )
            .map_err(|e| e.to_string())?;
            Ok(id)
        } else {
            conn.query_row(
                "INSERT INTO files (library_id, path, file_name, content_hash, status)
                 VALUES (?1,?2,?3,?4,'pending') RETURNING id",
                params![library_id, path, file_name, content_hash],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
        }
    }

    pub fn update_file_status(
        &self,
        file_id: i64,
        status: &str,
        error: Option<&str>,
        level: Option<i64>,
        chunks: Option<i64>,
    ) -> Result<(), String> {
        let conn = self.conn()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if status == "indexing" {
            conn.execute(
                "UPDATE files SET status=?1, error=?2, started_at=?3 WHERE id=?4",
                params![status, error, now, file_id],
            )
            .map_err(|e| e.to_string())?;
        } else if status == "done" || status == "error" || status == "canceled" {
            conn.execute(
                "UPDATE files SET status=?1, error=?2, indexed_level=COALESCE(?3,indexed_level),
                    chunks=COALESCE(?4,chunks), finished_at=?5 WHERE id=?6",
                params![status, error, level, chunks, now, file_id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            conn.execute(
                "UPDATE files SET status=?1, error=?2 WHERE id=?3",
                params![status, error, file_id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn get_files(&self, library_id: &str) -> Result<Vec<FileRecord>, String> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, library_id, path, file_name, content_hash, status,
                    indexed_level, chunks, error
             FROM files WHERE library_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![library_id], |row| {
                Ok(FileRecord {
                    id: row.get(0)?,
                    library_id: row.get(1)?,
                    path: row.get(2)?,
                    file_name: row.get(3)?,
                    content_hash: row.get(4)?,
                    status: row.get(5)?,
                    level: row.get::<_, i64>(6)?,
                    chunks: row.get::<_, i64>(7)?,
                    error: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    // ---- Team Mode: users / sessions / membership ----

    pub fn create_user(&self, id: &str, username: &str, pw_hash: &str, role: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO users (id, username, password_hash, role) VALUES (?1,?2,?3,?4)",
            params![id, username, pw_hash, role],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn find_user_by_username(&self, username: &str) -> Result<Option<(String, String, String)>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id, password_hash, role FROM users WHERE username = ?1")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![username], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.next().transpose().map_err(|e| e.to_string())?)
    }

    pub fn user_count(&self) -> Result<usize, String> {
        let conn = self.conn()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(count as usize)
    }

    pub fn list_users(&self) -> Result<Vec<(String, String, String)>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id, username, role FROM users ORDER BY username")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn set_user_role(&self, user_id: &str, role: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("UPDATE users SET role = ?1 WHERE id = ?2", params![user_id, role])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn add_session(&self, token: &str, user_id: &str, expires_at: i64) -> Result<(), String> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO sessions (token, user_id, created_at, expires_at) VALUES (?1,?2,?3,?4)",
            params![token, user_id, now, expires_at],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn user_for_token(&self, token: &str) -> Result<Option<(String, String)>, String> {
        let conn = self.conn()?;
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn
            .prepare(
                "SELECT u.id, u.role FROM sessions s JOIN users u ON u.id = s.user_id
                 WHERE s.token = ?1 AND s.expires_at > ?2",
            )
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![token, now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.next().transpose().map_err(|e| e.to_string())
    }

    pub fn delete_session(&self, token: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn grant_membership(&self, library_id: &str, user_id: &str, role: &str) -> Result<(), String> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO library_members (library_id, user_id, role) VALUES (?1,?2,?3)",
            params![library_id, user_id, role],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn library_role(&self, library_id: &str, user_id: &str) -> Result<Option<String>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT role FROM library_members WHERE library_id = ?1 AND user_id = ?2")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![library_id, user_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.next().transpose().map_err(|e| e.to_string())
    }

    pub fn list_files_libs(&self) -> Result<Vec<String>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT library_id FROM files ORDER BY library_id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_chunks_for_file(&self, file_id: i64) -> Result<Vec<(i64, i64, String)>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT chunk_index, level, content FROM chunks WHERE file_id = ?1 ORDER BY chunk_index")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![file_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn get_chunks_with_embeddings(
        &self,
        library_id: &str,
    ) -> Result<Vec<(i64, String, i64, String, Vec<f32>)>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT c.file_id, f.file_name, c.chunk_index, c.content, c.embedding
                 FROM chunks c JOIN files f ON f.id = c.file_id
                 WHERE c.library_id = ?1 AND c.embedding IS NOT NULL
                 ORDER BY c.file_id, c.chunk_index",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![library_id], |row| {
                let file_id: i64 = row.get(0)?;
                let file_name: String = row.get(1)?;
                let chunk_index: i64 = row.get(2)?;
                let content: String = row.get(3)?;
                let blob: Vec<u8> = row.get(4)?;
                let emb = blob_to_float_vec(&blob);
                Ok((file_id, file_name, chunk_index, content, emb))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn add_chunk(
        &self,
        library_id: &str,
        file_id: i64,
        file_name: &str,
        chunk_index: i64,
        content: &str,
        embedding: &[f32],
    ) -> Result<i64, String> {
        self.add_chunk_enriched(library_id, file_id, file_name, chunk_index, 1, content, Some(float_vec_to_blob(embedding)))
    }

    pub fn add_chunk_enriched(
        &self,
        library_id: &str,
        file_id: i64,
        file_name: &str,
        chunk_index: i64,
        level: i64,
        content: &str,
        embedding: Option<Vec<u8>>,
    ) -> Result<i64, String> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO chunks (library_id, file_id, file_name, chunk_index, level, content, embedding)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![library_id, file_id, file_name, chunk_index, level, content, embedding],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    /// In-Rust cosine similarity search (no SQLite extension needed).
    pub fn search(
        &self,
        library_id: &str,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<RetrievedChunk>, String> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, library_id, file_name, chunk_index, content, embedding
                 FROM chunks WHERE library_id = ?1 AND embedding IS NOT NULL",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![library_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut scored: Vec<(f32, RetrievedChunk)> = Vec::new();
        for r in rows {
            let (id, lib, name, idx, content, blob) = r.map_err(|e| e.to_string())?;
            let emb = blob_to_float_vec(&blob);
            if emb.len() == query_embedding.len() {
                let s = cosine(query_embedding, &emb);
                scored.push((
                    s,
                    RetrievedChunk {
                        id,
                        library_id: lib,
                        file_name: name,
                        chunk_index: idx,
                        content,
                        score: s,
                    },
                ));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored.into_iter().take(k).map(|(_, c)| c).collect())
    }

    pub fn file_count(&self, library_id: &str) -> Result<usize, String> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT COUNT(*) FROM files WHERE library_id = ?1",
            params![library_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

pub fn float_vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for &x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

pub fn blob_to_float_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
