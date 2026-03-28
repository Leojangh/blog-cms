use std::sync::Arc;
use rusqlite::Connection;
use tokio::sync::Mutex;

pub type Db = Arc<Mutex<Connection>>;

pub fn init() -> Db {
    let path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "blog.db".into());
    let conn = Connection::open(&path).expect("failed to open db");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        ALTER TABLE posts ADD COLUMN views INTEGER NOT NULL DEFAULT 0;"
    ).ok();
    // ensure table exists even if ALTER fails (column already exists)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            views INTEGER NOT NULL DEFAULT 0
        );"
    ).expect("failed to init db");
    Arc::new(Mutex::new(conn))
}
