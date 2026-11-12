//! Friend requests, friendships, blocks and reports.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::*;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db;
use crate::error::{AppError, ApiResult};
use crate::state::{AppState, RATE_FRIEND_REQUEST, RATE_REPORT};

// ---------------------------------------------------------------------------
// Friend requests
// ---------------------------------------------------------------------------

struct RequestRow {
    id: Uuid,
    sender_id: Uuid,
    receiver_id: Uuid,
    status: FriendRequestStatus,
    created_at: chrono::DateTime<chrono::Utc>,
}

async fn load_request(pool: &sqlx::PgPool, request_id: Uuid) -> ApiResult<Option<RequestRow>> {
    let row = sqlx::query(
        "SELECT id, sender_id, receiver_id, status, created_at FROM friend_requests WHERE id = $1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await?;
    let Some(r) = row else { return Ok(None) };
    Ok(Some(RequestRow {
        id: r.try_get(0)?,
        sender_id: r.try_get(1)?,
        receiver_id: r.try_get(2)?,
        status: parse_status(&r.try_get::<String, _>(3)?),
        created_at: r.try_get(4)?,
    }))
}

fn parse_status(s: &str) -> FriendRequestStatus {
    match s {
        "accepted" => FriendRequestStatus::Accepted,
        "declined" => FriendRequestStatus::Declined,
        "cancelled" => FriendRequestStatus::Cancelled,
        _ => FriendRequestStatus::Pending,
    }
}

fn status_db(s: FriendRequestStatus) -> &'static str {
    match s {
        FriendRequestStatus::Pending => "pending",
        FriendRequestStatus::Accepted => "accepted",
        FriendRequestStatus::Declined => "declined",
        FriendRequestStatus::Cancelled => "cancelled",
    }
}

async fn request_dto(
    state: &AppState,
    r: &RequestRow,
    viewer: Uuid,
) -> ApiResult<FriendRequest> {
    Ok(FriendRequest {
        id: r.id,
        sender: db::user_public(&state.pool, r.sender_id, Some(viewer)).await?,
        receiver: db::user_public(&state.pool, r.receiver_id, Some(viewer)).await?,
        status: r.status,
        created_at: r.created_at,
    })
}

pub async fn send_request(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<SendFriendRequestByUserId>,
) -> ApiResult<(axum::http::StatusCode, Json<FriendRequest>)> {
    state
        .limiter
        .check(&user.user_id.to_string(), &RATE_FRIEND_REQUEST)
        .map_err(AppError::rate_limited)?;

    let target = body.user_id;
    if target == user.user_id {
        return Err(AppError::validation("You can't send a request to yourself."));
    }
    if !db::user_exists(&state.pool, target).await? {
        return Err(AppError::not_found("That person doesn't exist (or left Nook)."));
    }
    if db::blocked_either(&state.pool, user.user_id, target).await? {
        return Err(AppError::forbidden(
            "You can't send a request right now because of a block between you.",
        ));
    }
    if db::are_friends(&state.pool, user.user_id, target).await? {
        return Err(AppError::conflict("already_friends", "You're already friends."));
    }

    // An incoming pending request means they want to be friends already.
    if let Some(existing) =
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM friend_requests WHERE sender_id = $2 AND receiver_id = $1 AND status = 'pending'",
        )
        .bind(user.user_id)
        .bind(target)
        .fetch_optional(&state.pool)
        .await?
    {
        let r = load_request(&state.pool, existing).await?.unwrap();
        return Ok((
            axum::http::StatusCode::OK,
            Json(request_dto(&state, &r, user.user_id).await?),
        ));
    }

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM friend_requests WHERE sender_id = $1 AND receiver_id = $2 AND status = 'pending'",
    )
    .bind(user.user_id)
    .bind(target)
    .fetch_optional(&state.pool)
    .await?;
    if let Some(id) = existing {
        let r = load_request(&state.pool, id).await?.unwrap();
        return Ok((
            axum::http::StatusCode::OK,
            Json(request_dto(&state, &r, user.user_id).await?),
        ));
    }

    let request_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO friend_requests (id, sender_id, receiver_id) VALUES ($1, $2, $3)",
    )
    .bind(request_id)
    .bind(user.user_id)
    .bind(target)
    .execute(&state.pool)
    .await?;

    let r = load_request(&state.pool, request_id).await?.unwrap();
    let dto = request_dto(&state, &r, user.user_id).await?;

    // Notify + push to the receiver.
    let target_settings = db::fetch_settings(&state.pool, target).await?;
    db::notify(
        &state,
        target,
        Some(user.user_id),
        "friend_request_received",
        format!("{} sent you a friend request", dto.sender.display_name),
        format!("@{}", dto.sender.username),
        None,
        None,
        serde_json::json!({ "request_id": request_id }),
        Some(target_settings.notify_friend_requests),
    )
    .await?;
    state.hub.send_to_user(target, &ServerEvent::FriendRequest { request: dto.clone() });
    // Sync the sender's other devices.
    state.hub.send_to_user(user.user_id, &ServerEvent::FriendRequest { request: dto.clone() });

    tracing::info!(from = %user.user_id, to = %target, "friend request sent");
    Ok((axum::http::StatusCode::CREATED, Json(dto)))
}

#[derive(serde::Serialize)]
pub struct RequestsResponse {
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

pub async fn list_requests(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<RequestsResponse>> {
    let rows = sqlx::query(
        r#"
        SELECT id, sender_id, receiver_id, created_at,
               (sender_id = $1) AS outgoing
        FROM friend_requests
        WHERE status = 'pending' AND (sender_id = $1 OR receiver_id = $1)
        ORDER BY created_at DESC
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;

    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    for row in rows {
        let r = RequestRow {
            id: row.try_get(0)?,
            sender_id: row.try_get(1)?,
            receiver_id: row.try_get(2)?,
            status: FriendRequestStatus::Pending,
            created_at: row.try_get(3)?,
        };
        let dto = request_dto(&state, &r, user.user_id).await?;
        if row.try_get::<bool, _>(4)? {
            outgoing.push(dto);
        } else {
            incoming.push(dto);
        }
    }
    Ok(Json(RequestsResponse { incoming, outgoing }))
}

pub async fn accept_request(
    State(state): State<AppState>,
    user: AuthUser,
    Path(request_id): Path<Uuid>,
) -> ApiResult<Json<FriendRequest>> {
    let r = load_request(&state.pool, request_id)
        .await?
        .ok_or_else(|| AppError::not_found("That request no longer exists."))?;
    if r.receiver_id != user.user_id {
        return Err(AppError::forbidden("This request wasn't sent to you."));
    }
    if r.status != FriendRequestStatus::Pending {
        return Err(AppError::conflict("not_pending", "That request is no longer pending."));
    }

    let (friendship_id, conversation_id) = {
        let mut tx = state.pool.begin().await?;
        sqlx::query(
            "UPDATE friend_requests SET status = 'accepted', responded_at = now() WHERE id = $1",
        )
        .bind(request_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO friendships (id, user_id_a, user_id_b) VALUES ($1, least($2, $3), greatest($2, $3))
             ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(r.sender_id)
        .bind(r.receiver_id)
        .execute(&mut *tx)
        .await?;

        // Private conversation, created once per pair.
        let dm_key = format!("{}|{}",
            r.sender_id.simple().to_string(),
            r.receiver_id.simple().to_string());
        // Canonical: sort the two ids lexicographically.
        let (a, b) = if r.sender_id < r.receiver_id {
            (r.sender_id, r.receiver_id)
        } else {
            (r.receiver_id, r.sender_id)
        };
        let dm_key = format!("{}|{}", a, b);

        let conversation_id: Uuid = sqlx::query_scalar(
            "INSERT INTO conversations (id, kind, dm_key, created_by)
             VALUES ($1, 'private', $2, NULL)
             ON CONFLICT (dm_key) DO UPDATE SET last_message_at = conversations.last_message_at
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(&dm_key)
        .fetch_one(&mut *tx)
        .await?;

        for uid in [a, b] {
            sqlx::query(
                "INSERT INTO conversation_members (id, conversation_id, user_id, role)
                 VALUES ($1, $2, $3, 'member')
                 ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(conversation_id)
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        (Uuid::now_v7(), conversation_id)
    };
    let _ = friendship_id;

    let updated = load_request(&state.pool, request_id).await?.unwrap();
    let dto = request_dto(&state, &updated, user.user_id).await?;

    // Notification for the sender (their request was accepted).
    let sender_settings = db::fetch_settings(&state.pool, r.sender_id).await?;
    db::notify(
        &state,
        r.sender_id,
        Some(user.user_id),
        "friend_request_accepted",
        format!("{} accepted your friend request", dto.receiver.display_name),
        "You are now friends — say hi!".to_string(),
        Some(conversation_id),
        None,
        serde_json::json!({ "request_id": request_id }),
        Some(sender_settings.notify_friend_requests),
    )
    .await?;

    let summary = conversation_summary_lite(&state, conversation_id, r.sender_id).await?;
    state.hub.send_to_user(
        r.sender_id,
        &ServerEvent::FriendRequestAccepted { request: dto.clone(), conversation_id },
    );
    if let Some(summary) = summary {
        state.hub.send_to_user(r.sender_id, &ServerEvent::ConversationUpdated { conversation: summary });
    }
    let summary2 = conversation_summary_lite(&state, conversation_id, r.receiver_id).await;
    state.hub.send_to_user(
        user.user_id,
        &ServerEvent::FriendRequestAccepted { request: dto.clone(), conversation_id },
    );
    if let Ok(Some(s)) = summary2 {
        state.hub.send_to_user(user.user_id, &ServerEvent::ConversationUpdated { conversation: s });
    }

    tracing::info!(sender = %r.sender_id, receiver = %r.receiver_id, "friend request accepted");
    Ok(Json(dto))
}

pub async fn decline_request(
    State(state): State<AppState>,
    user: AuthUser,
    Path(request_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let r = load_request(&state.pool, request_id)
        .await?
        .ok_or_else(|| AppError::not_found("That request no longer exists."))?;
    if r.receiver_id != user.user_id {
        return Err(AppError::forbidden("This request wasn't sent to you."));
    }
    if r.status != FriendRequestStatus::Pending {
        return Ok(StatusCode::NO_CONTENT);
    }
    sqlx::query("UPDATE friend_requests SET status = 'declined', responded_at = now() WHERE id = $1")
        .bind(request_id)
        .execute(&state.pool)
        .await?;
    state.hub.send_to_user(r.sender_id, &ServerEvent::FriendRequestClosed { request_id });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn cancel_request(
    State(state): State<AppState>,
    user: AuthUser,
    Path(request_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let r = load_request(&state.pool, request_id)
        .await?
        .ok_or_else(|| AppError::not_found("That request no longer exists."))?;
    if r.sender_id != user.user_id {
        return Err(AppError::forbidden("This request wasn't sent by you."));
    }
    if r.status != FriendRequestStatus::Pending {
        return Ok(StatusCode::NO_CONTENT);
    }
    sqlx::query("UPDATE friend_requests SET status = 'cancelled', responded_at = now() WHERE id = $1")
        .bind(request_id)
        .execute(&state.pool)
        .await?;
    state.hub.send_to_user(r.receiver_id, &ServerEvent::FriendRequestClosed { request_id });
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Friends list
// ---------------------------------------------------------------------------

pub async fn list_friends(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<Friend>>> {
    let rows = sqlx::query(
        r#"
        SELECT u.id, f.created_at
        FROM friendships f
        JOIN users u ON u.id = CASE WHEN f.user_id_a = $1 THEN f.user_id_b ELSE f.user_id_a END
        WHERE f.user_id_a = $1 OR f.user_id_b = $1
        ORDER BY u.display_name
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;

    let mut friends = Vec::with_capacity(rows.len());
    for row in rows {
        let friend_id: Uuid = row.try_get(0)?;
        let since: chrono::DateTime<chrono::Utc> = row.try_get(1)?;
        // Find the shared private conversation, if any.
        let (a, b) = if user.user_id < friend_id {
            (user.user_id, friend_id)
        } else {
            (friend_id, user.user_id)
        };
        let dm_key = format!("{}|{}", a, b);
        let conversation_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM conversations WHERE dm_key = $1",
        )
        .bind(&dm_key)
        .fetch_optional(&state.pool)
        .await?;
        friends.push(Friend {
            user: db::user_public(&state.pool, friend_id, Some(user.user_id)).await?,
            since,
            conversation_id,
        });
    }
    Ok(Json(friends))
}

pub async fn remove_friend(
    State(state): State<AppState>,
    user: AuthUser,
    Path(other): Path<Uuid>,
) -> ApiResult<StatusCode> {
    if !db::are_friends(&state.pool, user.user_id, other).await? {
        return Err(AppError::not_found("You're not friends with that person."));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "DELETE FROM friendships WHERE (user_id_a, user_id_b) = (least($1,$2), greatest($1,$2))",
    )
    .bind(user.user_id)
    .bind(other)
    .execute(&mut *tx)
    .await?;
    // Cancel any pending requests between them.
    sqlx::query(
        "UPDATE friend_requests SET status = 'cancelled', responded_at = now()
         WHERE status = 'pending'
           AND ((sender_id = $1 AND receiver_id = $2) OR (sender_id = $2 AND receiver_id = $1))",
    )
    .bind(user.user_id)
    .bind(other)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

pub async fn block_user(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<BlockRequest>,
) -> ApiResult<StatusCode> {
    let target = body.user_id;
    if target == user.user_id {
        return Err(AppError::validation("You can't block yourself."));
    }
    if !db::user_exists(&state.pool, target).await? {
        return Err(AppError::not_found("That person doesn't exist (or left Nook)."));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO blocked_users (blocker_id, blocked_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(user.user_id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    // Blocking ends the friendship and any pending requests in both directions.
    sqlx::query(
        "DELETE FROM friendships WHERE (user_id_a, user_id_b) = (least($1,$2), greatest($1,$2))",
    )
    .bind(user.user_id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE friend_requests SET status = 'cancelled', responded_at = now()
         WHERE status = 'pending'
           AND ((sender_id = $1 AND receiver_id = $2) OR (sender_id = $2 AND receiver_id = $1))",
    )
    .bind(user.user_id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_blocks(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<UserPublic>>> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT blocked_id FROM blocked_users WHERE blocker_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;
    let mut users = Vec::with_capacity(ids.len());
    for id in ids {
        users.push(db::user_public(&state.pool, id, None).await?);
    }
    Ok(Json(users))
}

pub async fn unblock_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(other): Path<Uuid>,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM blocked_users WHERE blocker_id = $1 AND blocked_id = $2")
        .bind(user.user_id)
        .bind(other)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

pub async fn report(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<ReportRequest>,
) -> ApiResult<StatusCode> {
    state
        .limiter
        .check(&user.user_id.to_string(), &RATE_REPORT)
        .map_err(AppError::rate_limited)?;

    validate_report_reason(&body.reason).map_err(AppError::validation)?;
    if body.target_user_id.is_none() && body.conversation_id.is_none() && body.message_id.is_none() {
        return Err(AppError::validation("Nothing to report — pick a message, chat or person."));
    }

    sqlx::query(
        "INSERT INTO reports (id, reporter_id, target_user_id, conversation_id, message_id, reason, details)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(user.user_id)
    .bind(body.target_user_id)
    .bind(body.conversation_id)
    .bind(body.message_id)
    .bind(body.reason.trim())
    .bind(body.details.trim())
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Compact summary for ConversationUpdated events.
pub(crate) async fn conversation_summary_lite(
    state: &AppState,
    conversation_id: Uuid,
    viewer: Uuid,
) -> ApiResult<Option<ConversationSummaryLite>> {
    let Some(base) = db::conversation_base(&state.pool, conversation_id).await? else {
        return Ok(None);
    };
    let title = match base.kind {
        db::ConvKind::Private => {
            let other = db::private_other_id(&state.pool, conversation_id, viewer).await?;
            match other {
                Some(o) => db::user_public(&state.pool, o, None).await?.display_name,
                None => base.title.clone().unwrap_or_default(),
            }
        }
        db::ConvKind::Group => base.title.clone().unwrap_or_default(),
    };
    Ok(Some(ConversationSummaryLite {
        id: base.id,
        kind: match base.kind {
            db::ConvKind::Private => ConversationKind::Private,
            db::ConvKind::Group => ConversationKind::Group,
        },
        title,
        member_count: db::member_count(&state.pool, conversation_id).await?,
        updated_at: base.last_message_at,
    }))
}
