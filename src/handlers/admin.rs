use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;

use crate::middleware::{clear_cookie, session_cookie};
use crate::AppState;

use std::sync::{Arc, LazyLock};

static ADMIN_USER: LazyLock<String> = LazyLock::new(|| {
    std::env::var("ADMIN_USER").unwrap_or_else(|_| "admin".into())
});
static ADMIN_PASS_HASH: LazyLock<String> = LazyLock::new(|| {
    std::env::var("ADMIN_PASS_HASH").expect("ADMIN_PASS_HASH env var required")
});

struct LoginState {
    failures: u32,
    last_attempt: Option<tokio::time::Instant>,
}

static LOGIN_STATE: LazyLock<Arc<Mutex<LoginState>>> =
    LazyLock::new(|| Arc::new(Mutex::new(LoginState { failures: 0, last_attempt: None })));

fn cooldown_secs(failures: u32) -> u64 {
    match failures {
        0..=2 => 0,
        3..=5 => 2,
        6..=10 => 5,
        _ => 30,
    }
}

fn verify_password(password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(&ADMIN_PASS_HASH) else { return false };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

fn verify_username(input: &str) -> bool {
    let a = input.as_bytes();
    let b = ADMIN_USER.as_bytes();
    let len = a.len().max(b.len());
    let mut a_padded = vec![0u8; len];
    let mut b_padded = vec![0u8; len];
    a_padded[..a.len()].copy_from_slice(a);
    b_padded[..b.len()].copy_from_slice(b);
    a.len() == b.len() && bool::from(a_padded.ct_eq(&b_padded))
}

// --- Auth ---

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

pub async fn login(Json(input): Json<LoginForm>) -> impl IntoResponse {
    {
        let state = LOGIN_STATE.lock().await;
        let cd = cooldown_secs(state.failures);
        if cd > 0 {
            if let Some(t) = state.last_attempt {
                if t.elapsed().as_secs() < cd {
                    return (StatusCode::TOO_MANY_REQUESTS, Json(serde_json::json!({"error": format!("请 {cd} 秒后再试")}))).into_response();
                }
            }
        }
    }

    if verify_username(&input.username) && verify_password(&input.password) {
        let mut state = LOGIN_STATE.lock().await;
        state.failures = 0;
        state.last_attempt = None;
        (StatusCode::OK, [(header::SET_COOKIE, session_cookie(&input.username))], Json(serde_json::json!({"ok": true}))).into_response()
    } else {
        let mut state = LOGIN_STATE.lock().await;
        state.failures = state.failures.saturating_add(1);
        state.last_attempt = Some(tokio::time::Instant::now());
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "用户名或密码错误"}))).into_response()
    }
}

pub async fn logout() -> impl IntoResponse {
    ([(header::SET_COOKIE, clear_cookie())], Json(serde_json::json!({"ok": true})))
}

pub async fn me(headers: axum::http::HeaderMap) -> impl IntoResponse {
    if crate::middleware::is_authed(&headers) {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    }
}

// --- Dashboard ---

#[derive(Serialize)]
struct DashboardPost {
    slug: String,
    title: String,
    views: i64,
    created_at: String,
}

#[derive(Serialize)]
struct DashboardData {
    posts: Vec<DashboardPost>,
    total_views: i64,
}

pub async fn dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let conn = state.db.lock().await;
    let total_views: i64 = conn
        .query_row("SELECT COALESCE(SUM(views),0) FROM posts", [], |r| r.get(0))
        .unwrap_or(0);
    let mut stmt = conn
        .prepare("SELECT slug, title, views, created_at FROM posts ORDER BY created_at DESC")
        .unwrap();
    let posts: Vec<DashboardPost> = stmt
        .query_map([], |row| {
            Ok(DashboardPost {
                slug: row.get(0)?,
                title: row.get(1)?,
                views: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(DashboardData { posts, total_views })
}

// --- CRUD ---

#[derive(Deserialize)]
pub struct PostInput {
    title: String,
    slug: String,
    content: String,
}

fn validate_slug(slug: &str) -> bool {
    !slug.is_empty() && slug.len() <= 128 && slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub async fn get_post(State(state): State<AppState>, Path(slug): Path<String>) -> impl IntoResponse {
    let conn = state.db.lock().await;
    let result = conn.query_row(
        "SELECT title, slug, content FROM posts WHERE slug = ?1",
        [&slug],
        |row| Ok(PostInput { title: row.get(0)?, slug: row.get(1)?, content: row.get(2)? }),
    );
    match result {
        Ok(p) => Json(serde_json::json!({"title": p.title, "slug": p.slug, "content": p.content})).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn create_post(State(state): State<AppState>, Json(input): Json<PostInput>) -> impl IntoResponse {
    if !validate_slug(&input.slug) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "slug 只能包含字母、数字和连字符"}))).into_response();
    }
    let conn = state.db.lock().await;
    match conn.execute(
        "INSERT INTO posts (title, slug, content) VALUES (?1, ?2, ?3)",
        (&input.title, &input.slug, &input.content),
    ) {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({"slug": input.slug}))).into_response(),
        Err(_) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "slug 已存在"}))).into_response(),
    }
}

pub async fn update_post(State(state): State<AppState>, Path(slug): Path<String>, Json(input): Json<PostInput>) -> impl IntoResponse {
    if !validate_slug(&input.slug) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "slug 只能包含字母、数字和连字符"}))).into_response();
    }
    let conn = state.db.lock().await;
    if let Ok(old_content) = conn.query_row(
        "SELECT content FROM posts WHERE slug = ?1", [&slug], |r| r.get::<_, String>(0),
    ) {
        let old_imgs = extract_image_urls(&old_content);
        let new_imgs = extract_image_urls(&input.content);
        for url in old_imgs.difference(&new_imgs) {
            delete_r2_object(&state.r2, url).await;
        }
    }
    match conn.execute(
        "UPDATE posts SET title = ?1, slug = ?2, content = ?3 WHERE slug = ?4",
        (&input.title, &input.slug, &input.content, &slug),
    ) {
        Ok(0) => StatusCode::NOT_FOUND.into_response(),
        Ok(_) => Json(serde_json::json!({"slug": input.slug})).into_response(),
        Err(_) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": "slug 已存在"}))).into_response(),
    }
}

pub async fn delete_post(State(state): State<AppState>, Path(slug): Path<String>) -> impl IntoResponse {
    let conn = state.db.lock().await;
    if let Ok(content) = conn.query_row(
        "SELECT content FROM posts WHERE slug = ?1", [&slug], |r| r.get::<_, String>(0),
    ) {
        for url in extract_image_urls(&content) {
            delete_r2_object(&state.r2, &url).await;
        }
    }
    conn.execute("DELETE FROM posts WHERE slug = ?1", [&slug]).ok();
    StatusCode::NO_CONTENT
}

// --- Images ---

use crate::r2::{R2_BUCKET, R2_PUBLIC_URL};

pub async fn upload_image(State(state): State<AppState>, mut multipart: Multipart) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() != Some("file") { continue; }
        let filename = field.file_name().unwrap_or("image.png").to_string();
        let ext = filename.rsplit('.').next().unwrap_or("png").to_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "不支持的图片格式"}))).into_response();
        }
        let Ok(data) = field.bytes().await else {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "读取失败"}))).into_response();
        };
        if data.len() > 5 * 1024 * 1024 {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "图片不能超过 5MB"}))).into_response();
        }
        let key = format!("{}.{ext}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f"));
        let content_type = match ext.as_str() {
            "png" => "image/png", "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif", "webp" => "image/webp", "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        };
        let result = state.r2.put_object()
            .bucket(R2_BUCKET.as_str())
            .key(&key)
            .body(data.into())
            .content_type(content_type)
            .send().await;
        if result.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let url = format!("{}/{key}", R2_PUBLIC_URL.as_str());
        return Json(serde_json::json!({"url": url})).into_response();
    }
    StatusCode::BAD_REQUEST.into_response()
}

#[derive(Deserialize)]
pub struct DeleteImageReq { url: String }

pub async fn delete_image(State(state): State<AppState>, Json(payload): Json<DeleteImageReq>) -> impl IntoResponse {
    delete_r2_object(&state.r2, &payload.url).await;
    StatusCode::NO_CONTENT
}

// --- Helpers ---

async fn delete_r2_object(r2: &aws_sdk_s3::Client, url: &str) {
    let prefix = format!("{}/", R2_PUBLIC_URL.as_str());
    let key = url.strip_prefix(&prefix).unwrap_or(url);
    if !key.is_empty() && !key.contains('/') && !key.contains("..") {
        r2.delete_object().bucket(R2_BUCKET.as_str()).key(key).send().await.ok();
    }
}

fn extract_image_urls(content: &str) -> std::collections::HashSet<String> {
    let prefix = R2_PUBLIC_URL.as_str();
    let mut urls = std::collections::HashSet::new();
    for cap in content.match_indices(prefix) {
        let rest = &content[cap.0..];
        let end = rest.find(|c: char| c == ')' || c == '"' || c == ' ' || c == '\n').unwrap_or(rest.len());
        urls.insert(rest[..end].to_string());
    }
    urls
}
