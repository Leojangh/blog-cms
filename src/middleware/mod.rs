use axum::{
    extract::Request,
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use cookie::Cookie;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use std::sync::LazyLock;
static SECRET: LazyLock<String> = LazyLock::new(|| {
    std::env::var("SESSION_SECRET").expect("SESSION_SECRET env var required")
});
const COOKIE_NAME: &str = "session";
const MAX_AGE_SECS: i64 = 86400;

pub fn sign(value: &str) -> String {
    let expires = chrono::Utc::now().timestamp() + MAX_AGE_SECS;
    let payload = format!("{value}:{expires}");
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("{payload}.{sig}")
}

pub fn verify(signed: &str) -> bool {
    let Some((payload, sig)) = signed.rsplit_once('.') else { return false };
    let mut mac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if !bool::from(expected.as_bytes().ct_eq(sig.as_bytes())) { return false }
    let Some((_, expires_str)) = payload.rsplit_once(':') else { return false };
    let Ok(expires) = expires_str.parse::<i64>() else { return false };
    chrono::Utc::now().timestamp() < expires
}

pub fn session_cookie(value: &str) -> String {
    Cookie::build((COOKIE_NAME, sign(value)))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(cookie::SameSite::None)
        .max_age(cookie::time::Duration::seconds(MAX_AGE_SECS))
        .to_string()
}

pub fn clear_cookie() -> String {
    Cookie::build((COOKIE_NAME, ""))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(cookie::SameSite::None)
        .max_age(cookie::time::Duration::ZERO)
        .to_string()
}

pub fn is_authed(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|s| Cookie::parse(s.trim().to_string()).ok())
        .any(|c| c.name() == COOKIE_NAME && verify(c.value()))
}

pub async fn require_auth(req: Request, next: Next) -> Response {
    if is_authed(req.headers()) {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

static ALLOWED_ORIGIN: LazyLock<String> = LazyLock::new(|| {
    std::env::var("CORS_ORIGIN").unwrap_or_else(|_| "https://genlz.com".into())
});

pub async fn cors(req: Request, next: Next) -> Response {
    let origin = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let is_preflight = req.method() == Method::OPTIONS;

    if is_preflight {
        let mut res = StatusCode::NO_CONTENT.into_response();
        let h = res.headers_mut();
        if let Some(ref o) = origin {
            if o == ALLOWED_ORIGIN.as_str() {
                h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(o).unwrap());
                h.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
                h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, HeaderValue::from_static("GET,POST,PUT,DELETE,OPTIONS"));
                h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, HeaderValue::from_static("content-type"));
                h.insert(header::ACCESS_CONTROL_MAX_AGE, HeaderValue::from_static("86400"));
            }
        }
        return res;
    }

    let mut res = next.run(req).await;
    if let Some(ref o) = origin {
        if o == ALLOWED_ORIGIN.as_str() {
            let h = res.headers_mut();
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_str(o).unwrap());
            h.insert(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, HeaderValue::from_static("true"));
        }
    }
    res
}

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("X-Content-Type-Options", HeaderValue::from_static("nosniff"));
    h.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    h.insert("Referrer-Policy", HeaderValue::from_static("strict-origin-when-cross-origin"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    res
}
