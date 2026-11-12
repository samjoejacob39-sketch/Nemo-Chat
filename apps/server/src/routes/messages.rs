//! Message history, sending, editing, deletion and reactions.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::*;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::{self, ConvKind};
use crate::error::{AppError, ApiResult};
use crate::routes::conversations::broadcast_message_update;
use crate::state::{AppState, RATE_MESSAGE};

#[derive(serde::Deserialize)]
pub struct HistoryQuery {
    pub before_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct HistoryResponse {
    pub messages: Vec<Message>,
    pub has_more: bool,
}

const HISTORY_LIMIT: i64 = 50;

pub async fn history(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<HistoryResponse>> {
    db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    let limit = q.limit.unwrap_or(HISTORY_LIMIT).clamp(1, 50);
    let ids = db::message_ids_page(&state.pool, conversation_id, user.user_id, q.before_id, limit).await?;
    let mut messages = Vec::with_capacity(ids.len());
    for id in &ids {
        messages.push(db::build_message(&state.pool, *id, user.user_id).await?);
    }
    let has_more = messages.len() as i64 == limit;
    Ok(Json(HistoryResponse { messages, has_more }))
}

pub async fn get_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(message_id): Path<Uuid>,
) -> ApiResult<Json<Message>> {
    let msg = db::load_message_row(&state.pool, message_id, user.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found."))?;
    db::require_membership(&state.pool, msg.conversation_id, user.user_id).await?;
    Ok(Json(db::build_message(&state.pool, message_id, user.user_id).await?))
}

pub async fn send_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> ApiResult<(StatusCode, Json<Message>)> {
    state
        .limiter
        .check(&user.user_id.to_string(), &RATE_MESSAGE)
        .map_err(AppError::rate_limited)?;

    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    let me = db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    let _ = me;

    match body.kind {
        MessageKind::Text => {
            if body.body.trim().is_empty() {
                return Err(AppError::validation("Your message is empty."));
            }
            validate_message_body(&body.body).map_err(AppError::validation)?;
        }
        MessageKind::Image => {
            let Some(att) = body.attachment_id else {
                return Err(AppError::validation("An image message needs an attachment."));
            };
            let owner: Option<Uuid> = sqlx::query_scalar("SELECT owner_id FROM attachments WHERE id = $1")
                .bind(att)
                .fetch_optional(&state.pool)
                .await?
                .flatten();
            if owner != Some(user.user_id) {
                return Err(AppError::validation("That image doesn't belong to you."));
            }
        }
        MessageKind::System => {
            return Err(AppError::forbidden("System messages are created by the server."));
        }
    }

    // Private chats require friendship and no block in either direction.
    if base.kind == ConvKind::Private {
        let other = db::private_other_id(&state.pool, conversation_id, user.user_id)
            .await?
            .ok_or_else(|| AppError::internal("private conversation missing participant"))?;
        if db::blocked_either(&state.pool, user.user_id, other).await? {
            return Err(AppError::forbidden("You can't send messages in this chat because of a block."));
        }
        if !db::are_friends(&state.pool, user.user_id, other).await? {
            return Err(AppError::forbidden(
                "You can only message friends. Send a friend request first.",
            ));
        }
    }

    // Reply target must live in the same conversation.
    if let Some(reply_to) = body.reply_to_id {
        let row: Option<Uuid> = sqlx::query_scalar(
            "SELECT conversation_id FROM messages WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(reply_to)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
        if row != Some(conversation_id) {
            return Err(AppError::validation("You can only reply to messages in this chat."));
        }
    }

    let message_id = Uuid::now_v7();
    let mut tx = state.pool.begin().await?;

    let body_text = body.body.trim().to_string();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, sender_id, kind, body, attachment_id, reply_to_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(user.user_id)
    .bind(match body.kind {
        MessageKind::Text => "text",
        MessageKind::Image => "image",
        MessageKind::System => "system",
    })
    .bind(&body_text)
    .bind(body.attachment_id)
    .bind(body.reply_to_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE conversations SET last_message_at = now() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    // The sender has, by definition, read up to their own message.
    sqlx::query(
        "UPDATE conversation_members SET last_read_message_id = $1 WHERE conversation_id = $2 AND user_id = $3",
    )
    .bind(message_id)
    .bind(conversation_id)
    .bind(user.user_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Broadcast to every member (sender included, for their other devices).
    let members = db::member_ids(&state.pool, conversation_id).await?;
    let mut offline_recipients = Vec::new();
    for uid in &members {
        let online = state.hub.is_online(*uid);
        if online {
            if let Ok(msg) = db::build_message(&state.pool, message_id, *uid).await {
                state.hub.send_to_user(*uid, &ServerEvent::MessageNew { message: msg });
            }
        } else if *uid != user.user_id {
            offline_recipients.push(*uid);
        }
    }

    // Offline members get a persistent notification (one per conversation).
    for uid in offline_recipients {
        let settings = db::fetch_settings(&state.pool, uid).await?;
        if !settings.notify_messages {
            continue;
        }
        // Replace any previous unread message notification for this chat.
        sqlx::query(
            "DELETE FROM notifications WHERE user_id = $1 AND conversation_id = $2 AND kind = 'message' AND read_at IS NULL",
        )
        .bind(uid)
        .bind(conversation_id)
        .execute(&state.pool)
        .await?;

        let sender_name = crate::routes::conversations::display_name_of(&state, user.user_id).await?;
        let chat_label = match base.kind {
            ConvKind::Group => format!("{} \u{00b7} {}", base.title.clone().unwrap_or_else(|| "Group".into()), sender_name),
            ConvKind::Private => sender_name,
        };
        let preview = match body.kind {
            MessageKind::Image => "\u{1f4f7} Photo".to_string(),
            _ => body_text.chars().take(80).collect::<String>(),
        };
        db::notify(
            &state,
            uid,
            Some(user.user_id),
            "message",
            chat_label,
            preview,
            Some(conversation_id),
            Some(message_id),
            serde_json::json!({}),
            None,
        )
        .await?;
    }

    let message = db::build_message(&state.pool, message_id, user.user_id).await?;
    Ok((StatusCode::CREATED, Json(message)))
}

pub async fn edit_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(message_id): Path<Uuid>,
    Json(body): Json<EditMessageRequest>,
) -> ApiResult<Json<Message>> {
    let row = db::load_message_row(&state.pool, message_id, user.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found."))?;
    db::require_membership(&state.pool, row.conversation_id, user.user_id).await?;

    if row.sender.id != user.user_id {
        return Err(AppError::forbidden("You can only edit your own messages."));
    }
    if row.kind != MessageKind::Text {
        return Err(AppError::validation("Only text messages can be edited."));
    }
    if !nook_shared::can_edit(row.created_at) {
        return Err(AppError::validation("You can edit a message for 15 minutes after sending."));
    }
    let text = body.body.trim().to_string();
    if text.is_empty() {
        return Err(AppError::validation("Your message is empty."));
    }
    validate_message_body(&text).map_err(AppError::validation)?;

    sqlx::query("UPDATE messages SET body = $1, edited_at = now() WHERE id = $2")
        .bind(&text)
        .bind(message_id)
        .execute(&state.pool)
        .await?;

    broadcast_message_update(&state, row.conversation_id, message_id).await?;
    Ok(Json(db::build_message(&state.pool, message_id, user.user_id).await?))
}

/// `DELETE /api/messages/:id?scope=me|everyone`
pub async fn delete_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(message_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<StatusCode> {
    let scope = match params.get("scope").map(|s| s.as_str()).unwrap_or("me") {
        "me" => DeleteScope::Me,
        "everyone" => DeleteScope::Everyone,
        _ => return Err(AppError::bad_request("scope must be 'me' or 'everyone'")),
    };
    let row = db::load_message_row(&state.pool, message_id, user.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found (or already gone)."))?;
    let conversation_id = row.conversation_id;
    let me = db::require_membership(&state.pool, conversation_id, user.user_id).await?;

    match scope {
        DeleteScope::Me => {
            sqlx::query(
                "INSERT INTO message_hides (user_id, message_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(user.user_id)
            .bind(message_id)
            .execute(&state.pool)
            .await?;
            Ok(StatusCode::NO_CONTENT)
        }
        DeleteScope::Everyone => {
            let is_admin = matches!(me.role, MemberRole::Owner | MemberRole::Admin);
            let own_recent = row.sender.id == user.user_id && nook_shared::can_delete_for_everyone(row.created_at);
            if !is_admin && !own_recent {
                return Err(AppError::forbidden(
                    "You can delete your own messages for everyone within an hour of sending.",
                ));
            }
            let mut tx = state.pool.begin().await?;
            sqlx::query("UPDATE messages SET deleted_at = now() WHERE id = $1")
                .bind(message_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM message_reactions WHERE message_id = $1")
                .bind(message_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;

            let members = db::member_ids(&state.pool, conversation_id).await?;
            state.hub.send_to_users(
                members.iter().copied(),
                &ServerEvent::MessageDeleted { conversation_id, message_id },
            );
            Ok(StatusCode::NO_CONTENT)
        }
    }
}

pub async fn set_reaction(
    State(state): State<AppState>,
    user: AuthUser,
    Path(message_id): Path<Uuid>,
    Json(body): Json<SetReactionRequest>,
) -> ApiResult<Json<Message>> {
    state
        .limiter
        .check(&user.user_id.to_string(), &crate::state::RATE_REACTION)
        .map_err(AppError::rate_limited)?;

    let row = db::load_message_row(&state.pool, message_id, user.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found."))?;
    db::require_membership(&state.pool, row.conversation_id, user.user_id).await?;

    if !nook_shared::is_valid_reaction(&body.emoji) {
        return Err(AppError::validation("That reaction isn't available."));
    }

    sqlx::query(
        "INSERT INTO message_reactions (message_id, user_id, emoji) VALUES ($1, $2, $3)
         ON CONFLICT (message_id, user_id) DO UPDATE SET emoji = $3",
    )
    .bind(message_id)
    .bind(user.user_id)
    .bind(&body.emoji)
    .execute(&state.pool)
    .await?;

    broadcast_message_update(&state, row.conversation_id, message_id).await?;
    Ok(Json(db::build_message(&state.pool, message_id, user.user_id).await?))
}

pub async fn remove_reaction(
    State(state): State<AppState>,
    user: AuthUser,
    Path(message_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let row = db::load_message_row(&state.pool, message_id, user.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found."))?;
    db::require_membership(&state.pool, row.conversation_id, user.user_id).await?;

    sqlx::query("DELETE FROM message_reactions WHERE message_id = $1 AND user_id = $2")
        .bind(message_id)
        .bind(user.user_id)
        .execute(&state.pool)
        .await?;

    broadcast_message_update(&state, row.conversation_id, message_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
