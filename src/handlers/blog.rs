use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::AppState;

static VIEW_COUNTER: std::sync::LazyLock<Mutex<HashMap<String, i64>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn flush_views(db: &crate::models::Db) {
    let mut counts = VIEW_COUNTER.lock().unwrap();
    if counts.is_empty() { return; }
    let db = db.blocking_lock();
    for (slug, count) in counts.drain() {
        db.execute("UPDATE posts SET views = views + ?1 WHERE slug = ?2", (&count, &slug)).ok();
    }
}

pub fn start_flush_task(db: crate::models::Db) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            flush_views(&db);
        }
    });
}

#[derive(Serialize)]
struct PostItem { slug: String, title: String, created_at: String }

#[derive(Serialize)]
struct PostDetail { slug: String, title: String, content: String, created_at: String }

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.db.lock().await;
    let mut stmt = conn.prepare("SELECT slug, title, created_at FROM posts ORDER BY created_at DESC").unwrap();
    let posts: Vec<PostItem> = stmt
        .query_map([], |row| Ok(PostItem { slug: row.get(0)?, title: row.get(1)?, created_at: row.get(2)? }))
        .unwrap().filter_map(|r| r.ok()).collect();
    Json(posts)
}

pub async fn show(State(state): State<AppState>, Path(slug): Path<String>) -> impl IntoResponse {
    { VIEW_COUNTER.lock().unwrap().entry(slug.clone()).and_modify(|c| *c += 1).or_insert(1); }
    let conn = state.db.lock().await;
    let result = conn.query_row(
        "SELECT slug, title, content, created_at FROM posts WHERE slug = ?1", [&slug],
        |row| Ok(PostDetail { slug: row.get(0)?, title: row.get(1)?, content: row.get(2)?, created_at: row.get(3)? }),
    );
    match result {
        Ok(post) => Json(post).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
