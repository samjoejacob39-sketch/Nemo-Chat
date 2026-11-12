//! Endpoints for the acting user's profile, settings, password, avatar,
//! onboarding acceptance and account deletion.

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::*;
use uuid::Uuid;

use crate::auth::{self, AuthUser};
use crate::db;
use crate::error::{AppError, ApiResult};
use crate::media;
use crate::state::AppState;

#[derive(serde::Serialize)]
pub struct MeResponse {
    pub user: UserPublic,
    pub settings: UserSettings,
    pub terms: TermsStatus,
}

pub async fn get_me(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<MeResponse>> {
    let me = db::me_public(&state, user.user_id).await?;
    let settings = db::fetch_settings(&state.pool, user.user_id).await?;
    let terms = crate::routes::auth::terms_status_row(
        &state.pool,
        user.user_id,
        &state.config.policy_version,
    )
    .await?;
    Ok(Json(MeResponse { user: me, settings, terms }))
}

pub async fn update_me(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> ApiResult<Json<UserPublic>> {
    if let Some(name) = &body.display_name {
        let trimmed = name.trim().to_string();
        validate_display_name(&trimmed).map_err(AppError::validation)?;
        sqlx::query("UPDATE users SET display_name = $1, updated_at = now() WHERE id = $2")
            .bind(&trimmed)
            .bind(user.user_id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(bio) = &body.bio {
        validate_bio(bio).map_err(AppError::validation)?;
        sqlx::query("UPDATE users SET bio = $1, updated_at = now() WHERE id = $2")
            .bind(bio.trim())
            .bind(user.user_id)
            .execute(&state.pool)
            .await?;
    }
    Ok(Json(db::me_public(&state, user.user_id).await?))
}

/// `PATCH /api/me/avatar` — multipart field `file`.
pub async fn update_avatar(
    State(state): State<AppState>,
    user: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<UserPublic>> {
    state
        .limiter
        .check(&user.user_id.to_string(), &crate::state::RATE_UPLOAD)
        .map_err(AppError::rate_limited)?;

    let mut image: Option<media::StoredImage> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("invalid multipart body: {e}")))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(format!("could not read upload: {e}")))?;
            image = Some(
                media::store_image(&state, user.user_id, "avatar", &bytes).await?,
            );
        }
    }

    let stored = image.ok_or_else(|| AppError::validation("Please choose an image file."))?;
    sqlx::query("UPDATE users SET avatar_attachment_id = $1, updated_at = now() WHERE id = $2")
        .bind(stored.attachment.id)
        .bind(user.user_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(db::me_public(&state, user.user_id).await?))
}

pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<ChangePasswordRequest>,
) -> ApiResult<StatusCode> {
    validate_password(&body.new_password).map_err(|e| AppError::validation(e.to_string()))?;

    let current: Option<String> = sqlx::query_scalar(
        "SELECT password_hash FROM users WHERE id = $1",
    )
    .bind(user.user_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    let Some(hash) = current else {
        return Err(AppError::not_found("Account not found."));
    };
    if !auth::verify_password(&body.current_password, &hash) {
        return Err(AppError::validation("Your current password is incorrect."));
    }

    let new_hash = auth::hash_password(&body.new_password)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = now() WHERE id = $2")
        .bind(&new_hash)
        .bind(user.user_id)
        .execute(&mut *tx)
        .await?;
    // Keep the current session, sign out everywhere else.
    sqlx::query(
        "UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL AND id <> $2",
    )
    .bind(user.user_id)
    .bind(user.session_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/me` — deletes the account and all associated data.
pub async fn delete_account(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<DeleteAccountRequest>,
) -> ApiResult<StatusCode> {
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(user.user_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    let Some(hash) = hash else {
        return Err(AppError::not_found("Account not found."));
    };
    if !auth::verify_password(&body.password, &hash) {
        return Err(AppError::validation("That password is incorrect."));
    }

    // Best-effort removal of stored media files.
    if let Ok(paths) = sqlx::query_scalar::<_, String>(
        "SELECT storage_path FROM attachments WHERE owner_id = $1",
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await
    {
        for p in paths {
            let full = state.config.media_dir.join(&p);
            let _ = tokio::fs::remove_file(full).await;
        }
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user.user_id)
        .execute(&state.pool)
        .await?;
    tracing::info!(user_id = %user.user_id, "account deleted");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_settings(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<UserSettings>> {
    Ok(Json(db::fetch_settings(&state.pool, user.user_id).await?))
}

pub async fn update_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<UpdateSettingsRequest>,
) -> ApiResult<Json<UserSettings>> {
    fn vis(v: Visibility) -> String {
        match v {
            Visibility::Everyone => "everyone".into(),
            Visibility::Friends => "friends".into(),
            Visibility::Nobody => "nobody".into(),
        }
    }
    fn theme(t: Theme) -> String {
        match t {
            Theme::System => "system".into(),
            Theme::Light => "light".into(),
            Theme::Dark => "dark".into(),
        }
    }

    let mut tx = state.pool.begin().await?;
    if let Some(v) = body.online_visibility {
        sqlx::query("UPDATE user_settings SET online_visibility = $1, updated_at = now() WHERE user_id = $2")
            .bind(vis(v))
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(v) = body.last_seen_visibility {
        sqlx::query("UPDATE user_settings SET last_seen_visibility = $1, updated_at = now() WHERE user_id = $2")
            .bind(vis(v))
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(b) = body.read_receipts {
        sqlx::query("UPDATE user_settings SET read_receipts = $1, updated_at = now() WHERE user_id = $2")
            .bind(b)
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(v) = body.searchability {
        sqlx::query("UPDATE user_settings SET searchability = $1, updated_at = now() WHERE user_id = $2")
            .bind(vis(v))
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(t) = body.theme {
        sqlx::query("UPDATE user_settings SET theme = $1, updated_at = now() WHERE user_id = $2")
            .bind(theme(t))
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(b) = body.notify_friend_requests {
        sqlx::query("UPDATE user_settings SET notify_friend_requests = $1, updated_at = now() WHERE user_id = $2")
            .bind(b)
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(b) = body.notify_messages {
        sqlx::query("UPDATE user_settings SET notify_messages = $1, updated_at = now() WHERE user_id = $2")
            .bind(b)
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(b) = body.notify_group_events {
        sqlx::query("UPDATE user_settings SET notify_group_events = $1, updated_at = now() WHERE user_id = $2")
            .bind(b)
            .bind(user.user_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    Ok(Json(db::fetch_settings(&state.pool, user.user_id).await?))
}

/// `POST /api/me/recovery-codes/regenerate` — replaces the account's
/// recovery code and returns the new one exactly once. Any previous code
/// stops working immediately.
pub async fn regenerate_recovery_code(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<RecoveryCodeResponse>> {
    let code = auth::generate_recovery_code();
    let hash = auth::hash_password(&auth::normalize_recovery_code(&code))?;

    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO recovery_codes (id, user_id, code_hash) VALUES ($1, $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(user.user_id)
    .bind(&hash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    tracing::info!(user_id = %user.user_id, "recovery code regenerated");
    Ok(Json(RecoveryCodeResponse { code }))
}

pub async fn terms_status(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<TermsStatus>> {
    let status =
        crate::routes::auth::terms_status_row(&state.pool, user.user_id, &state.config.policy_version).await?;
    Ok(Json(status))
}

pub async fn accept_terms(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<AcceptTermsRequest>,
) -> ApiResult<StatusCode> {
    crate::routes::auth::ensure_current_version(&state.config, &body.version)?;
    let _ = Uuid::now_v7;
    sqlx::query(
        "INSERT INTO terms_acceptances (id, user_id, policy_version) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, policy_version) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(user.user_id)
    .bind(&body.version)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
