//! Authentication endpoints: register, login, recovery, logout.

use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::policies;
use nook_shared::*;
use sqlx::Row;
use std::net::SocketAddr;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::db;
use crate::error::{AppError, ApiResult};
use crate::state::{AppState, RATE_AUTH};

/// Whether `user` has accepted the current policy version.
pub async fn onboarding_required(pool: &sqlx::PgPool, user_id: Uuid, version: &str) -> ApiResult<bool> {
    let latest: Option<String> = sqlx::query_scalar(
        r#"
        SELECT policy_version FROM terms_acceptances
        WHERE user_id = $1
        ORDER BY accepted_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(latest.as_deref() != Some(version))
}

pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RegisterRequest>,
) -> ApiResult<(StatusCode, Json<AuthResponse>)> {
    state
        .limiter
        .check(&addr.ip().to_string(), &RATE_AUTH)
        .map_err(AppError::rate_limited)?;

    let username = normalize_username(&body.username);
    let display_name = body.display_name.trim().to_string();

    if let Err(e) = validate_display_name(&display_name) {
        return Err(AppError::validation(e));
    }
    if let Err(e) = validate_username(&username) {
        return Err(AppError::validation(e.to_string()));
    }
    if let Err(e) = validate_password(&body.password) {
        return Err(AppError::validation(e.to_string()));
    }

    let password_hash = auth::hash_password(&body.password)?;
    let user_id = Uuid::now_v7();
    let recovery_code = auth::generate_recovery_code();
    let recovery_hash = auth::hash_password(&auth::normalize_recovery_code(&recovery_code))?;

    let mut tx = state.pool.begin().await?;

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE lower(username) = lower($1))",
    )
    .bind(&username)
    .fetch_one(&mut *tx)
    .await?;
    if exists {
        return Err(AppError::conflict(
            "username_taken",
            "That username is already taken. Try another one.",
        ));
    }

    sqlx::query(
        "INSERT INTO users (id, username, display_name, password_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(&username)
    .bind(&display_name)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO user_settings (user_id) VALUES ($1)")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "INSERT INTO recovery_codes (id, user_id, code_hash) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(&recovery_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let (session_id, token) = auth::create_session(&state.pool, user_id, "registration", state.config.session_days).await?;
    let _ = session_id;
    let user = db::me_public(&state, user_id).await?;

    tracing::info!(%user_id, %username, "user registered");

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            session: SessionInfo { token, user },
            onboarding_required: true,
            recovery_code: Some(recovery_code),
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    state
        .limiter
        .check(&addr.ip().to_string(), &RATE_AUTH)
        .map_err(AppError::rate_limited)?;

    let username = normalize_username(&body.username);
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE lower(username) = $1",
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await?;

    let Some((user_id, password_hash)) = row else {
        // Same generic message for unknown username and wrong password.
        return Err(AppError::validation("Username or password is incorrect."));
    };

    if !auth::verify_password(&body.password, &password_hash) {
        tracing::warn!(%username, "failed login attempt");
        return Err(AppError::validation("Username or password is incorrect."));
    }

    let (session_id, token) = auth::create_session(&state.pool, user_id, "", state.config.session_days).await?;
    let _ = session_id;
    let user = db::me_public(&state, user_id).await?;
    let onboarding = onboarding_required(&state.pool, user_id, &state.config.policy_version).await?;

    Ok(Json(AuthResponse {
        session: SessionInfo { token, user },
        onboarding_required: onboarding,
        recovery_code: None,
    }))
}

pub async fn recover(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RecoverRequest>,
) -> ApiResult<Json<AuthResponse>> {
    state
        .limiter
        .check(&addr.ip().to_string(), &RATE_AUTH)
        .map_err(AppError::rate_limited)?;

    let username = normalize_username(&body.username);
    let code = auth::normalize_recovery_code(&body.recovery_code);
    if code.is_empty() {
        return Err(AppError::validation("Please enter your recovery code."));
    }
    if let Err(e) = validate_password(&body.new_password) {
        return Err(AppError::validation(e.to_string()));
    }

    let row: Option<(Uuid, String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT rc.user_id, rc.code_hash, rc.used_at
         FROM recovery_codes rc JOIN users u ON u.id = rc.user_id
         WHERE lower(u.username) = $1",
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await?;

    let Some((user_id, code_hash, used_at)) = row else {
        return Err(AppError::validation("Username or recovery code is incorrect."));
    };
    if used_at.is_some() {
        return Err(AppError::conflict(
            "recovery_used",
            "That recovery code was already used. Contact the server operator for help.",
        ));
    }
    if !auth::verify_password(&code, &code_hash) {
        return Err(AppError::validation("Username or recovery code is incorrect."));
    }

    let new_hash = auth::hash_password(&body.new_password)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE recovery_codes SET used_at = now() WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    // Password reset invalidates every existing session.
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let (_, token) = auth::create_session(&state.pool, user_id, "recovery", state.config.session_days).await?;
    let user = db::me_public(&state, user_id).await?;
    let onboarding = onboarding_required(&state.pool, user_id, &state.config.policy_version).await?;

    tracing::info!(%user_id, "account recovered via recovery code");

    Ok(Json(AuthResponse {
        session: SessionInfo { token, user },
        onboarding_required: onboarding,
        recovery_code: None,
    }))
}

pub async fn logout(State(state): State<AppState>, user: AuthUser) -> ApiResult<StatusCode> {
    auth::revoke_session(&state.pool, user.session_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Helper for `GET /api/me`.
pub(crate) async fn terms_status_row(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    version: &str,
) -> ApiResult<TermsStatus> {
    let row = sqlx::query(
        r#"
        SELECT policy_version, accepted_at FROM terms_acceptances
        WHERE user_id = $1
        ORDER BY accepted_at DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let (latest_accepted_version, accepted_at) = match row {
        Some(r) => (Some(r.try_get::<String, _>(0)?), Some(r.try_get::<chrono::DateTime<chrono::Utc>, _>(1)?)),
        None => (None, None),
    };
    let onboarding_required = latest_accepted_version.as_deref() != Some(version);
    Ok(TermsStatus {
        current_version: version.to_string(),
        latest_accepted_version,
        onboarding_required,
        accepted_at,
    })
}

/// Referenced by me.rs to keep the policy version consistent.
pub(crate) fn policy_version(config: &crate::config::Config) -> &str {
    &config.policy_version
}

/// Ensures the referenced version matches the current policy version.
pub(crate) fn ensure_current_version(config: &crate::config::Config, requested: &str) -> ApiResult<()> {
    if requested != policies::POLICY_VERSION && requested != config.policy_version {
        return Err(AppError::validation(
            "That policy version is not current. Please reload the onboarding screen.",
        ));
    }
    Ok(())
}
