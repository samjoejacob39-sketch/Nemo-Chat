//! Username search and public profile lookup.

use axum::extract::{Path, Query, State};
use axum::Json;
use nook_shared::*;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db;
use crate::db::USER_SELECT;
use crate::error::{AppError, ApiResult};
use crate::state::{AppState, RATE_SEARCH};

#[derive(serde::Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

pub async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    state
        .limiter
        .check(&user.user_id.to_string(), &RATE_SEARCH)
        .map_err(AppError::rate_limited)?;

    let query = normalize_username(&q.q);
    let query = query.trim_start_matches('@').to_string();
    if query.is_empty() {
        return Ok(Json(SearchResponse { results: vec![], query: q.q }));
    }
    if query.chars().count() > SEARCH_QUERY_MAX {
        return Err(AppError::validation(format!(
            "Search queries can be at most {SEARCH_QUERY_MAX} characters."
        )));
    }

    // USER_SELECT is a trusted compile-time constant; all user input is bound.
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"
        SELECT {USER_SELECT}
        FROM users u
        LEFT JOIN attachments a ON a.id = u.avatar_attachment_id
        JOIN user_settings s ON s.user_id = u.id
        WHERE u.id <> $1
          -- blocks in either direction hide a user from search
          AND NOT EXISTS (
            SELECT 1 FROM blocked_users b
            WHERE (b.blocker_id = $1 AND b.blocked_id = u.id)
               OR (b.blocker_id = u.id AND b.blocked_id = $1)
          )
          -- respect the target's searchability preference
          AND (
            s.searchability = 'everyone'
            OR (s.searchability = 'friends' AND EXISTS (
                SELECT 1 FROM friendships f
                WHERE (f.user_id_a, f.user_id_b) = (least(u.id, $1), greatest(u.id, $1))
            ))
          )
          AND (
            u.username ILIKE $2 || '%'
            OR u.username ILIKE '%' || $2 || '%'
            OR u.display_name ILIKE '%' || $2 || '%'
          )
        ORDER BY
          (u.username = lower($2)) DESC,
          (u.username ILIKE $2 || '%') DESC,
          char_length(u.username),
          u.username
        LIMIT 20
        "#,
    )))
    .bind(user.user_id)
    .bind(query)
    .fetch_all(&state.pool)
    .await?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: Uuid = row.try_get("id")?;
        let mut avatar: Option<AttachmentMeta> = None;
        if let Some(av_id) = row.try_get::<Option<Uuid>, _>("av_id")? {
            avatar = Some(AttachmentMeta {
                id: av_id,
                mime: row.try_get::<Option<String>, _>("av_mime")?.unwrap_or_else(|| "image/jpeg".into()),
                byte_size: row.try_get::<Option<i64>, _>("av_size")?.unwrap_or(0),
                width: row.try_get::<Option<i32>, _>("av_width")?.map(|x| x as u32),
                height: row.try_get::<Option<i32>, _>("av_height")?.map(|x| x as u32),
                url: format!("/api/media/{av_id}"),
            });
        }
        let relationship = db::relationship(&state.pool, user.user_id, id).await?;
        results.push(UserPublic {
            id,
            username: row.try_get("username")?,
            display_name: row.try_get("display_name")?,
            bio: row.try_get("bio")?,
            avatar,
            relationship: Some(relationship),
        });
    }

    Ok(Json(SearchResponse { results, query: q.q }))
}

pub async fn get_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<UserPublic>> {
    let blocked = db::blocked_either(&state.pool, user.user_id, user_id).await?;
    let friends = db::are_friends(&state.pool, user.user_id, user_id).await?;
    if blocked && !friends {
        return Err(AppError::not_found("That person doesn't exist (or left Nook)."));
    }
    Ok(Json(db::user_public(&state.pool, user_id, Some(user.user_id)).await?))
}
