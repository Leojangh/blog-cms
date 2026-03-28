mod handlers;
mod middleware;
mod models;
mod r2;

use axum::{middleware::from_fn, routing::{get, post}, Router};
use tower_http::limit::RequestBodyLimitLayer;
use tracing_subscriber::fmt;

pub type AppState = std::sync::Arc<AppStateInner>;

pub struct AppStateInner {
    pub db: models::Db,
    pub r2: aws_sdk_s3::Client,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    fmt::init();

    let db = models::init();
    let r2 = r2::client().await;

    handlers::blog::start_flush_task(db.clone());

    let state = std::sync::Arc::new(AppStateInner { db, r2 });

    let admin = Router::new()
        .route("/api/admin/dashboard", get(handlers::admin::dashboard))
        .route("/api/admin/posts", post(handlers::admin::create_post))
        .route("/api/admin/posts/{slug}", get(handlers::admin::get_post).put(handlers::admin::update_post).delete(handlers::admin::delete_post))
        .route("/api/admin/upload", post(handlers::admin::upload_image).layer(RequestBodyLimitLayer::new(6 * 1024 * 1024)))
        .route("/api/admin/delete-image", post(handlers::admin::delete_image))
        .layer(from_fn(middleware::require_auth));

    let app = Router::new()
        .route("/api/posts", get(handlers::blog::list))
        .route("/api/posts/{slug}", get(handlers::blog::show))
        .route("/api/auth/login", post(handlers::admin::login))
        .route("/api/auth/logout", post(handlers::admin::logout))
        .route("/api/auth/me", get(handlers::admin::me))
        .merge(admin)
        .layer(RequestBodyLimitLayer::new(512 * 1024))
        .layer(from_fn(middleware::cors))
        .layer(from_fn(middleware::security_headers))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    tracing::info!("listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}
