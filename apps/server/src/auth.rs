//! Password hashing (Argon2id), session tokens, recovery codes and the
//! authenticated-user extractor.

use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use argon2::password_hash::phc;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use chrono::{Duration, Utc};
use getrandom::fill as fill_random;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, ApiResult};

pub fn hash_password(password: &str) -> ApiResult<String> {
    // argon2 0.6 generates a fresh 16-byte salt internally.
    let hash = Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| AppError::internal(format!("hash failure: {e}")))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    phc::PasswordHash::new(stored)
        .ok()
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex::encode(digest)
}

/// 32 random bytes → 64-char hex session token.
pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    fill_random(&mut buf).expect("system RNG failure");
    hex::encode(buf)
}

// ---------------------------------------------------------------------------
// Recovery codes: crockford-ish base32, formatted `NOOK-XXXX-XXXX-XXXX`.
// ---------------------------------------------------------------------------

const CODE_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn generate_recovery_code() -> String {
    let mut buf = [0u8; 12];
    fill_random(&mut buf).expect("system RNG failure");
    let chars: String = buf
        .iter()
        .map(|b| CODE_ALPHABET[(*b as usize) % CODE_ALPHABET.len()] as char)
        .collect();
    format!("NOOK-{}-{}-{}", &chars[0..4], &chars[4..8], &chars[8..12])
}

/// Recovery codes are accepted case-insensitively and ignore dashes/spaces.
pub fn normalize_recovery_code(raw: &str) -> String {
    raw.trim()
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

const TOKEN_PREFIX: &str = "Bearer ";

/// Extracts and validates the session token from request headers.
/// Returns the authenticated user + session ids.
pub async fn authenticate(pool: &PgPool, parts: &Parts) -> ApiResult<AuthUser> {
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(AppError::unauthorized)?;
    let token = header
        .strip_prefix(TOKEN_PREFIX)
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| {
            AppError::unauthorized()
        })?;
    authenticate_token(pool, token.trim()).await
}

pub async fn authenticate_token(pool: &PgPool, token: &str) -> ApiResult<AuthUser> {
    let token_hash = sha256_hex(token.as_bytes());
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT s.id, s.user_id
        FROM sessions s
        WHERE s.token_hash = $1
          AND s.revoked_at IS NULL
          AND s.expires_at > now()
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(AppError::unauthorized)?;

    // Throttled activity bookkeeping: at most once per minute per session.
    let _ = sqlx::query(
        "UPDATE sessions SET last_seen_at = now() WHERE id = $1 AND last_seen_at < now() - interval '60 seconds'",
    )
    .bind(row.0)
    .execute(pool)
    .await;

    Ok(AuthUser { user_id: row.1, session_id: row.0 })
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    device: &str,
    days: i64,
) -> ApiResult<(Uuid, String)> {
    let token = generate_token();
    let token_hash = sha256_hex(token.as_bytes());
    let session_id = Uuid::now_v7();
    let expires_at = Utc::now() + Duration::days(days);
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, device, expires_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&token_hash)
    .bind(device)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok((session_id, token))
}

pub async fn revoke_session(pool: &PgPool, session_id: Uuid) -> ApiResult<()> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_all_sessions(pool: &PgPool, user_id: Uuid) -> ApiResult<()> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
