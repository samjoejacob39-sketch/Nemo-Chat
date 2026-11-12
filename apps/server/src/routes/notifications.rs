//! Notification center endpoints.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::*;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db;
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct ListQuery {
    pub before_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct NotificationsResponse {
    pub notifications: Vec<Notification>,
    pub unread_count: u32,
    pub has_more: bool,
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<NotificationsResponse>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id FROM notifications
        WHERE user_id = $1 AND ($2::uuid IS NULL OR id < $2)
        ORDER BY id DESC
        LIMIT $3
        "#,
    )
    .bind(user.user_id)
    .bind(q.before_id)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let mut notifications = Vec::with_capacity(rows.len());
    for id in &rows {
        notifications.push(db::notification_to_dto(&state.pool, *id, user.user_id).await?);
    }
    let unread_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.user_id)
    .fetch_one(&state.pool)
    .await? as u32;
    let has_more = notifications.len() as i64 == limit;

    Ok(Json(NotificationsResponse { notifications, unread_count, has_more }))
}

pub async fn mark_read(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<MarkNotificationsReadRequest>,
) -> ApiResult<StatusCode> {
    if body.ids.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }
    sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND id = ANY($2) AND read_at IS NULL",
    )
    .bind(user.user_id)
    .bind(&body.ids)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn mark_all_read(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<StatusCode> {
    sqlx::query(
        "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.user_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
