//! Conversations: listing, private chats, groups, membership and permissions.

use axum::extract::{FromRequest, Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use nook_shared::*;
use sqlx::Row;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::{self, ConvBase, ConvKind};
use crate::error::{AppError, ApiResult};
use crate::media;
use crate::routes::friends::conversation_summary_lite;
use crate::state::{AppState, RATE_MESSAGE};

// ---------------------------------------------------------------------------
// Listing
// ---------------------------------------------------------------------------

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<ConversationSummary>>> {
    // 1) My member rows + conversation bases in one query.
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.kind, c.title, c.description,
               ca.id, ca.mime, ca.byte_size, ca.width, ca.height,
               c.created_by, c.created_at, c.last_message_at,
               cm.role, cm.muted_at, cm.pinned_at, cm.last_read_message_id, cm.joined_at
        FROM conversation_members cm
        JOIN conversations c ON c.id = cm.conversation_id
        LEFT JOIN attachments ca ON ca.id = c.avatar_attachment_id
        WHERE cm.user_id = $1
        ORDER BY cm.pinned_at DESC NULLS LAST, c.last_message_at DESC
        LIMIT 200
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.pool)
    .await?;

    let conv_ids: Vec<Uuid> = rows.iter().map(|r| r.try_get::<Uuid, _>(0)).collect::<Result<_, _>>()?;

    // 2) Other participants of private chats, in one query.
    struct OtherInfo {
        other_id: Uuid,
    }
    let mut others: std::collections::HashMap<Uuid, OtherInfo> = std::collections::HashMap::new();
    if !conv_ids.is_empty() {
        let other_rows = sqlx::query(
            r#"
            SELECT cm.conversation_id, cm.user_id
            FROM conversation_members cm
            WHERE cm.conversation_id = ANY($1) AND cm.user_id <> $2
            "#,
        )
        .bind(&conv_ids)
        .bind(user.user_id)
        .fetch_all(&state.pool)
        .await?;
        for r in other_rows {
            let cid: Uuid = r.try_get(0)?;
            let uid: Uuid = r.try_get(1)?;
            others.insert(cid, OtherInfo { other_id: uid });
        }
    }

    // 3) Last message ids per conversation, in one query.
    let mut last_ids: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();
    if !conv_ids.is_empty() {
        let lm_rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (m.conversation_id) m.conversation_id, m.id
            FROM messages m
            WHERE m.conversation_id = ANY($1)
              AND m.deleted_at IS NULL
              AND NOT EXISTS (SELECT 1 FROM message_hides h WHERE h.user_id = $2 AND h.message_id = m.id)
            ORDER BY m.conversation_id, m.id DESC
            "#,
        )
        .bind(&conv_ids)
        .bind(user.user_id)
        .fetch_all(&state.pool)
        .await?;
        for r in lm_rows {
            last_ids.insert(r.try_get(0)?, r.try_get(1)?);
        }
    }

    // 4) Unread counts in one grouped query.
    let mut unreads: std::collections::HashMap<Uuid, u32> = std::collections::HashMap::new();
    if !conv_ids.is_empty() {
        let unread_rows = sqlx::query(
            r#"
            SELECT m.conversation_id, count(*)::int AS unread
            FROM messages m
            JOIN conversation_members cm
              ON cm.conversation_id = m.conversation_id AND cm.user_id = $2
            LEFT JOIN message_hides h ON h.user_id = $2 AND h.message_id = m.id
            WHERE m.conversation_id = ANY($1)
              AND m.sender_id <> $2
              AND m.deleted_at IS NULL
              AND m.kind <> 'system'
              AND h.message_id IS NULL
              AND (cm.last_read_message_id IS NULL OR m.id > cm.last_read_message_id)
            GROUP BY m.conversation_id
            "#,
        )
        .bind(&conv_ids)
        .bind(user.user_id)
        .fetch_all(&state.pool)
        .await?;
        for r in unread_rows {
            unreads.insert(r.try_get(0)?, r.try_get::<i32, _>(1)? as u32);
        }
    }

    // 5) Assemble.
    let mut summaries = Vec::with_capacity(rows.len());
    for row in &rows {
        let id: Uuid = row.try_get(0)?;
        let base = db::ConvBase {
            id,
            kind: db::parse_kind(&row.try_get::<String, _>(1)?),
            title: row.try_get(2)?,
            description: row.try_get(3)?,
            avatar: {
                let av_id: Option<Uuid> = row.try_get(4)?;
                match av_id {
                    Some(aid) => Some(AttachmentMeta {
                        id: aid,
                        mime: row.try_get::<Option<String>, _>(5)?.unwrap_or_else(|| "image/jpeg".into()),
                        byte_size: row.try_get::<Option<i64>, _>(6)?.unwrap_or(0),
                        width: row.try_get::<Option<i32>, _>(7)?.map(|x| x as u32),
                        height: row.try_get::<Option<i32>, _>(8)?.map(|x| x as u32),
                        url: format!("/api/media/{aid}"),
                    }),
                    None => None,
                }
            },
            created_by: row.try_get(9)?,
            created_at: row.try_get(10)?,
            last_message_at: row.try_get(11)?,
        };
        let member = db::MemberRow {
            conversation_id: id,
            user_id: user.user_id,
            role: db::parse_role(&row.try_get::<String, _>(12)?),
            muted_at: row.try_get(13)?,
            pinned_at: row.try_get(14)?,
            last_read_message_id: row.try_get(15)?,
            joined_at: row.try_get(16)?,
        };

        let (other_user, title, other_online) = match base.kind {
            ConvKind::Private => {
                let Some(info) = others.get(&id) else {
                    continue; // broken private conversation
                };
                let other = db::user_public(&state.pool, info.other_id, Some(user.user_id)).await?;
                let online = db::presence_visible_to(&state.pool, &state.hub, info.other_id, user.user_id).await;
                let title = other.display_name.clone();
                (Some(other), title, Some(online))
            }
            ConvKind::Group => (None, base.title.clone().unwrap_or_default(), None),
        };

        let last_message = match last_ids.get(&id) {
            Some(mid) => db::build_message(&state.pool, *mid, user.user_id).await.ok(),
            None => None,
        };

        summaries.push(ConversationSummary {
            id,
            kind: match base.kind {
                ConvKind::Private => ConversationKind::Private,
                ConvKind::Group => ConversationKind::Group,
            },
            title,
            description: if base.description.is_empty() { None } else { Some(base.description.clone()) },
            avatar: base.avatar.clone(),
            other_user,
            last_message,
            unread_count: unreads.get(&id).copied().unwrap_or(0),
            pinned: member.pinned_at.is_some(),
            muted: member.muted_at.is_some(),
            member_count: db::member_count(&state.pool, id).await?,
            other_online,
            updated_at: base.last_message_at,
        });
    }

    Ok(Json(summaries))
}

// ---------------------------------------------------------------------------
// Private conversations
// ---------------------------------------------------------------------------

pub async fn start_private(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<StartPrivateRequest>,
) -> ApiResult<(StatusCode, Json<ConversationSummary>)> {
    let other = body.user_id;
    if other == user.user_id {
        return Err(AppError::validation("You can't start a chat with yourself."));
    }
    if !db::user_exists(&state.pool, other).await? {
        return Err(AppError::not_found("That person doesn't exist (or left Nook)."));
    }
    if db::blocked_either(&state.pool, user.user_id, other).await? {
        return Err(AppError::forbidden("You can't message this person because of a block."));
    }
    if !db::are_friends(&state.pool, user.user_id, other).await? {
        return Err(AppError::forbidden(
            "You can only message friends. Send a friend request first.",
        ));
    }

    let (a, b) = if user.user_id < other { (user.user_id, other) } else { (other, user.user_id) };
    let dm_key = format!("{}|{}", a, b);

    let conversation_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM conversations WHERE dm_key = $1",
    )
    .bind(&dm_key)
    .fetch_optional(&state.pool)
    .await?;

    let conversation_id = match conversation_id {
        Some(id) => id,
        None => {
            let mut tx = state.pool.begin().await?;
            let id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO conversations (id, kind, dm_key) VALUES ($1, 'private', $2)",
            )
            .bind(id)
            .bind(&dm_key)
            .execute(&mut *tx)
            .await?;
            for uid in [a, b] {
                sqlx::query(
                    "INSERT INTO conversation_members (id, conversation_id, user_id, role) VALUES ($1, $2, $3, 'member')",
                )
                .bind(Uuid::now_v7())
                .bind(id)
                .bind(uid)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            id
        }
    };

    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::internal("conversation missing"))?;
    let member = db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    let summary = db::conversation_summary(&state, &base, &member, user.user_id).await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

// ---------------------------------------------------------------------------
// Group creation
// ---------------------------------------------------------------------------

/// `POST /api/conversations/group` — JSON body + optional multipart avatar.
/// Accepts either `application/json` or `multipart/form-data` (field `file`
/// for the avatar + text fields `name`, `description`, `member_ids`).
pub async fn create_group(
    State(state): State<AppState>,
    user: AuthUser,
    request: axum::extract::Request,
) -> ApiResult<(StatusCode, Json<ConversationDetail>)> {
    state
        .limiter
        .check(&user.user_id.to_string(), &crate::state::RATE_UPLOAD)
        .map_err(AppError::rate_limited)?;

    let content_type = request
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let (name, description, member_ids, avatar_attachment_id) = if content_type.starts_with("multipart/form-data") {
        let mut multipart = Multipart::from_request(request, &state)
            .await
            .map_err(|_| AppError::bad_request("invalid multipart body"))?;
        let mut name = String::new();
        let mut description = String::new();
        let mut member_ids: Vec<Uuid> = Vec::new();
        let mut avatar_attachment_id: Option<Uuid> = None;
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::bad_request(format!("invalid multipart body: {e}")))?
        {
            match field.name() {
                Some("name") => name = field.text().await.unwrap_or_default(),
                Some("description") => description = field.text().await.unwrap_or_default(),
                Some("member_ids") => {
                    let raw = field.text().await.unwrap_or_default();
                    for part in raw.split(',') {
                        if let Ok(id) = part.trim().parse::<Uuid>() {
                            member_ids.push(id);
                        }
                    }
                }
                Some("file") => {
                    let bytes = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::bad_request(format!("could not read upload: {e}")))?;
                    let stored = media::store_image(&state, user.user_id, "image", &bytes).await?;
                    avatar_attachment_id = Some(stored.attachment.id);
                }
                _ => {}
            }
        }
        (name, description, member_ids, avatar_attachment_id)
    } else {
        let body: CreateGroupRequest = serde_json::from_slice(
            &axum::body::to_bytes(request.into_body(), state.config.max_upload_bytes)
                .await
                .map_err(|_| AppError::bad_request("invalid body"))?,
        )
        .map_err(|_| AppError::bad_request("invalid JSON body"))?;
        (body.name, body.description.unwrap_or_default(), body.member_ids, body.avatar_attachment_id)
    };

    validate_group_title(&name).map_err(AppError::validation)?;
    validate_bio(&description).map_err(AppError::validation)?;

    // Members must be deduplicated existing friends (and not blocked pairs).
    let mut ids: Vec<Uuid> = member_ids.clone();
    ids.sort();
    ids.dedup();
    ids.retain(|&id| id != user.user_id);
    for id in &ids {
        if !db::user_exists(&state.pool, *id).await? {
            return Err(AppError::validation("One of the selected people no longer exists."));
        }
        if !db::are_friends(&state.pool, user.user_id, *id).await? {
            return Err(AppError::validation(
                "Groups can only include people who are already your friends.",
            ));
        }
        if db::blocked_either(&state.pool, user.user_id, *id).await? {
            return Err(AppError::validation(
                "You have a block with one of the selected people.",
            ));
        }
    }

    // Validate avatar ownership.
    if let Some(av) = avatar_attachment_id {
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT owner_id FROM attachments WHERE id = $1")
            .bind(av)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
        if owner != Some(user.user_id) {
            return Err(AppError::validation("That image doesn't belong to you."));
        }
    }

    let conversation_id = Uuid::now_v7();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO conversations (id, kind, title, description, avatar_attachment_id, created_by)
         VALUES ($1, 'group', $2, $3, $4, $5)",
    )
    .bind(conversation_id)
    .bind(name.trim())
    .bind(description.trim())
    .bind(avatar_attachment_id)
    .bind(user.user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO conversation_members (id, conversation_id, user_id, role) VALUES ($1, $2, $3, 'owner')",
    )
    .bind(Uuid::now_v7())
    .bind(conversation_id)
    .bind(user.user_id)
    .execute(&mut *tx)
    .await?;

    for id in &ids {
        sqlx::query(
            "INSERT INTO conversation_members (id, conversation_id, user_id, role) VALUES ($1, $2, $3, 'member')",
        )
        .bind(Uuid::now_v7())
        .bind(conversation_id)
        .bind(*id)
        .execute(&mut *tx)
        .await?;
        let _ = Uuid::now_v7();
    }

    // System message marking creation.
    let system_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, sender_id, kind, body) VALUES ($1, $2, $3, 'system', $4)",
    )
    .bind(system_id)
    .bind(conversation_id)
    .bind(user.user_id)
    .bind(format!("{} created the group \u{201c}{}\u{201d}", display_name_of(&state, user.user_id).await?, name.trim()))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // Notify each added member.
    let my_name = display_name_of(&state, user.user_id).await?;
    for id in &ids {
        let settings = db::fetch_settings(&state.pool, *id).await?;
        db::notify(
            &state,
            *id,
            Some(user.user_id),
            "added_to_group",
            format!("{my_name} added you to \u{201c}{}\u{201d}", name.trim()),
            "Take a look when you get a chance.".to_string(),
            Some(conversation_id),
            None,
            serde_json::json!({}),
            Some(settings.notify_group_events),
        )
        .await?;
        state.hub.send_to_user(*id, &ServerEvent::MemberAdded { conversation_id });
    }
    if let Some(s) = conversation_summary_lite(&state, conversation_id, user.user_id).await? {
        for uid in [user.user_id].into_iter().chain(ids.iter().copied()) {
            state.hub.send_to_user(uid, &ServerEvent::ConversationUpdated { conversation: s.clone() });
        }
    }

    let detail = build_detail(&state, conversation_id, user.user_id).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

pub async fn display_name_of(state: &AppState, user_id: Uuid) -> ApiResult<String> {
    Ok(sqlx::query_scalar::<_, String>("SELECT display_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .unwrap_or_else(|| "Someone".into()))
}

// ---------------------------------------------------------------------------
// Detail / update
// ---------------------------------------------------------------------------

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<ConversationDetail>> {
    // membership check happens inside build_detail
    Ok(Json(build_detail(&state, conversation_id, user.user_id).await?))
}

pub async fn build_detail(
    state: &AppState,
    conversation_id: Uuid,
    viewer: Uuid,
) -> ApiResult<ConversationDetail> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    let member = db::require_membership(&state.pool, conversation_id, viewer).await?;

    // Members with profile + presence.
    let member_rows = sqlx::query(
        r#"
        SELECT cm.user_id, cm.role, cm.joined_at
        FROM conversation_members cm
        WHERE cm.conversation_id = $1
        ORDER BY cm.joined_at
        "#,
    )
    .bind(conversation_id)
    .fetch_all(&state.pool)
    .await?;

    let mut members = Vec::with_capacity(member_rows.len());
    for row in &member_rows {
        let uid: Uuid = row.try_get(0)?;
        let role = db::parse_role(&row.try_get::<String, _>(1)?);
        let joined_at: chrono::DateTime<chrono::Utc> = row.try_get(2)?;
        let online = db::presence_visible_to(&state.pool, &state.hub, uid, viewer).await;
        members.push(ConversationMember {
            user: db::user_public(&state.pool, uid, Some(viewer)).await?,
            role,
            joined_at,
            online,
        });
    }

    // Pinned message (one per conversation).
    let pinned_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM messages WHERE conversation_id = $1 AND pinned_at IS NOT NULL AND deleted_at IS NULL ORDER BY pinned_at DESC LIMIT 1",
    )
    .bind(conversation_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    let pinned_message = match pinned_id {
        Some(pid) => db::build_message(&state.pool, pid, viewer).await.ok(),
        None => None,
    };

    let summary = db::conversation_summary(state, &base, &member, viewer).await?;

    Ok(ConversationDetail {
        summary,
        members,
        my_role: member.role,
        pinned_message,
        created_at: base.created_at,
    })
}

pub async fn update_conversation(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    mut multipart: Multipart,
) -> ApiResult<Json<ConversationDetail>> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    let member = db::require_membership(&state.pool, conversation_id, user.user_id).await?;

    if base.kind == ConvKind::Group {
        if !matches!(member.role, MemberRole::Owner | MemberRole::Admin) {
            return Err(AppError::forbidden("Only group admins can change group details."));
        }
    } else {
        return Err(AppError::forbidden("Private chats can't be renamed here."));
    }

    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut avatar_attachment_id: Option<Uuid> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("invalid multipart body: {e}")))?
    {
        match field.name() {
            Some("title") => title = Some(field.text().await.unwrap_or_default()),
            Some("description") => description = Some(field.text().await.unwrap_or_default()),
            Some("file") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("could not read upload: {e}")))?;
                let stored = media::store_image(&state, user.user_id, "image", &bytes).await?;
                avatar_attachment_id = Some(stored.attachment.id);
            }
            _ => {}
        }
    }

    if let Some(t) = &title {
        validate_group_title(t).map_err(AppError::validation)?;
    }
    if let Some(d) = &description {
        validate_bio(d).map_err(AppError::validation)?;
    }

    if let Some(t) = &title {
        sqlx::query("UPDATE conversations SET title = $1 WHERE id = $2")
            .bind(t.trim())
            .bind(conversation_id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(d) = &description {
        sqlx::query("UPDATE conversations SET description = $1 WHERE id = $2")
            .bind(d.trim())
            .bind(conversation_id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(av) = avatar_attachment_id {
        sqlx::query("UPDATE conversations SET avatar_attachment_id = $1 WHERE id = $2")
            .bind(av)
            .bind(conversation_id)
            .execute(&state.pool)
            .await?;
    }

    // Push updates to all members.
    let ids = db::member_ids(&state.pool, conversation_id).await?;
    if let Some(s) = conversation_summary_lite(&state, conversation_id, user.user_id).await? {
        state.hub.send_to_users(ids.iter().copied(), &ServerEvent::ConversationUpdated { conversation: s });
    }
    for uid in &ids {
        state.hub.send_to_user(*uid, &ServerEvent::MemberAdded { conversation_id });
    }

    Ok(Json(build_detail(&state, conversation_id, user.user_id).await?))
}

// ---------------------------------------------------------------------------
// Membership management
// ---------------------------------------------------------------------------

async fn insert_system_message(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: Uuid,
    actor: Uuid,
    text: &str,
) -> ApiResult<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO messages (id, conversation_id, sender_id, kind, body) VALUES ($1, $2, $3, 'system', $4)",
    )
    .bind(id)
    .bind(conversation_id)
    .bind(actor)
    .bind(text)
    .execute(&mut **tx)
    .await?;
    let _ = state;
    Ok(id)
}

async fn require_group_admin(state: &AppState, conversation_id: Uuid, user_id: Uuid) -> ApiResult<(ConvBase, db::MemberRow)> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    if base.kind != ConvKind::Group {
        return Err(AppError::forbidden("That's not a group."));
    }
    let member = db::require_membership(&state.pool, conversation_id, user_id).await?;
    if !matches!(member.role, MemberRole::Owner | MemberRole::Admin) {
        return Err(AppError::forbidden("Only group admins can do that."));
    }
    Ok((base, member))
}

pub async fn add_members(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<AddMembersRequest>,
) -> ApiResult<Json<ConversationDetail>> {
    let (base, _me) = require_group_admin(&state, conversation_id, user.user_id).await?;

    let mut ids = body.user_ids.clone();
    ids.sort();
    ids.dedup();
    ids.retain(|id| *id != user.user_id);
    if ids.is_empty() {
        return Err(AppError::validation("Pick at least one friend to add."));
    }

    let existing: Vec<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id = $1 AND user_id = ANY($2)",
    )
    .bind(conversation_id)
    .bind(&ids)
    .fetch_all(&state.pool)
    .await?;
    ids.retain(|id| !existing.contains(id));

    for id in &ids {
        if !db::user_exists(&state.pool, *id).await? {
            return Err(AppError::validation("One of the selected people no longer exists."));
        }
        if !db::are_friends(&state.pool, user.user_id, *id).await? {
            return Err(AppError::validation(
                "You can only add people who are already your friends.",
            ));
        }
    }

    let my_name = display_name_of(&state, user.user_id).await?;
    let mut added_names = Vec::new();
    {
        let mut tx = state.pool.begin().await?;
        for id in &ids {
            sqlx::query(
                "INSERT INTO conversation_members (id, conversation_id, user_id, role) VALUES ($1, $2, $3, 'member') ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(conversation_id)
            .bind(*id)
            .execute(&mut *tx)
            .await?;
            let name = display_name_of(&state, *id).await?;
            added_names.push(name.clone());
            insert_system_message(&state, &mut tx, conversation_id, user.user_id, &format!("{my_name} added {name}")).await?;
        }
        tx.commit().await?;
    }

    for id in &ids {
        let settings = db::fetch_settings(&state.pool, *id).await?;
        db::notify(
            &state,
            *id,
            Some(user.user_id),
            "added_to_group",
            format!("{my_name} added you to \u{201c}{}\u{201d}", base.title.clone().unwrap_or_default()),
            String::new(),
            Some(conversation_id),
            None,
            serde_json::json!({}),
            Some(settings.notify_group_events),
        )
        .await?;
        state.hub.send_to_user(*id, &ServerEvent::MemberAdded { conversation_id });
    }

    let all = db::member_ids(&state.pool, conversation_id).await?;
    if let Some(s) = conversation_summary_lite(&state, conversation_id, user.user_id).await? {
        state.hub.send_to_users(all.iter().copied(), &ServerEvent::ConversationUpdated { conversation: s });
    }
    for uid in &all {
        if *uid != user.user_id {
            state.hub.send_to_user(*uid, &ServerEvent::MemberAdded { conversation_id });
        }
    }

    Ok(Json(build_detail(&state, conversation_id, user.user_id).await?))
}

pub async fn remove_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path((conversation_id, target)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<ConversationDetail>> {
    let (base, me) = require_group_admin(&state, conversation_id, user.user_id).await?;
    let target_member = db::require_membership(&state.pool, conversation_id, target).await?;
    if target_member.role == MemberRole::Owner {
        return Err(AppError::forbidden("The group owner can't be removed."));
    }
    // Admins can only remove regular members; only the owner removes admins.
    if target_member.role == MemberRole::Admin && me.role != MemberRole::Owner {
        return Err(AppError::forbidden("Only the group owner can remove admins."));
    }

    let target_name = display_name_of(&state, target).await?;
    let my_name = display_name_of(&state, user.user_id).await?;
    {
        let mut tx = state.pool.begin().await?;
        sqlx::query("DELETE FROM conversation_members WHERE conversation_id = $1 AND user_id = $2")
            .bind(conversation_id)
            .bind(target)
            .execute(&mut *tx)
            .await?;
        insert_system_message(&state, &mut tx, conversation_id, user.user_id, &format!("{my_name} removed {target_name}")).await?;
        tx.commit().await?;
    }

    let settings = db::fetch_settings(&state.pool, target).await?;
    db::notify(
        &state,
        target,
        Some(user.user_id),
        "removed_from_group",
        format!("You were removed from \u{201c}{}\u{201d}", base.title.clone().unwrap_or_default()),
        String::new(),
        Some(conversation_id),
        None,
        serde_json::json!({}),
        Some(settings.notify_group_events),
    )
    .await?;
    state.hub.send_to_user(target, &ServerEvent::MemberRemoved { conversation_id, user_id: target });

    let all = db::member_ids(&state.pool, conversation_id).await?;
    for uid in &all {
        state.hub.send_to_user(*uid, &ServerEvent::MemberRemoved { conversation_id, user_id: target });
    }
    if let Some(s) = conversation_summary_lite(&state, conversation_id, user.user_id).await? {
        state.hub.send_to_users(all.iter().copied(), &ServerEvent::ConversationUpdated { conversation: s });
    }

    Ok(Json(build_detail(&state, conversation_id, user.user_id).await?))
}

pub async fn leave_group(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    if base.kind != ConvKind::Group {
        return Err(AppError::forbidden("You can't leave a private chat — remove the friend instead."));
    }
    let me = db::require_membership(&state.pool, conversation_id, user.user_id).await?;

    let my_name = display_name_of(&state, user.user_id).await?;
    let mut tx = state.pool.begin().await?;

    // Owner leaving: transfer to the oldest admin, else the oldest member.
    if me.role == MemberRole::Owner {
        let successor: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT user_id FROM conversation_members
            WHERE conversation_id = $1 AND role = 'admin'
            ORDER BY joined_at LIMIT 1
            "#,
        )
        .bind(conversation_id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        let successor = match successor {
            Some(s) => Some(s),
            None => {
                sqlx::query_scalar(
                    r#"
                    SELECT user_id FROM conversation_members
                    WHERE conversation_id = $1 AND user_id <> $2
                    ORDER BY joined_at LIMIT 1
                    "#,
                )
                .bind(conversation_id)
                .bind(user.user_id)
                .fetch_optional(&mut *tx)
                .await?
                .flatten()
            }
        };
        if let Some(s) = successor {
            sqlx::query("UPDATE conversation_members SET role = 'owner' WHERE conversation_id = $1 AND user_id = $2")
                .bind(conversation_id)
                .bind(s)
                .execute(&mut *tx)
                .await?;
            insert_system_message(&state, &mut tx, conversation_id, user.user_id, &format!("{my_name} made {} the owner", display_name_of(&state, s).await?)).await?;
        }
    }

    sqlx::query("DELETE FROM conversation_members WHERE conversation_id = $1 AND user_id = $2")
        .bind(conversation_id)
        .bind(user.user_id)
        .execute(&mut *tx)
        .await?;
    insert_system_message(&state, &mut tx, conversation_id, user.user_id, &format!("{my_name} left")).await?;
    tx.commit().await?;

    // If nobody remains, delete the group entirely.
    let remaining = db::member_count(&state.pool, conversation_id).await?;
    if remaining == 0 {
        sqlx::query("DELETE FROM conversations WHERE id = $1").bind(conversation_id).execute(&state.pool).await?;
    }

    state.hub.send_to_user(user.user_id, &ServerEvent::MemberRemoved { conversation_id, user_id: user.user_id });
    let settings = db::fetch_settings(&state.pool, user.user_id).await?;
    let _ = settings;
    if remaining > 0 {
        let all = db::member_ids(&state.pool, conversation_id).await?;
        for uid in &all {
            state.hub.send_to_user(*uid, &ServerEvent::MemberRemoved { conversation_id, user_id: user.user_id });
        }
        if let Some(s) = conversation_summary_lite(&state, conversation_id, user.user_id).await? {
            state.hub.send_to_users(all.iter().copied(), &ServerEvent::ConversationUpdated { conversation: s });
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Pin / mute
// ---------------------------------------------------------------------------

pub async fn pin_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<MarkReadRequest>,
) -> ApiResult<StatusCode> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    let me = db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    if base.kind == ConvKind::Group && !matches!(me.role, MemberRole::Owner | MemberRole::Admin) {
        return Err(AppError::forbidden("Only group admins can pin messages."));
    }
    // Message must exist in this conversation.
    let msg_conv: Option<Uuid> = sqlx::query_scalar("SELECT conversation_id FROM messages WHERE id = $1 AND deleted_at IS NULL")
        .bind(body.message_id)
        .fetch_optional(&state.pool)
        .await?
        .flatten();
    if msg_conv != Some(conversation_id) {
        return Err(AppError::not_found("Message not found in this chat."));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE messages SET pinned_at = NULL WHERE conversation_id = $1 AND pinned_at IS NOT NULL")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE messages SET pinned_at = now() WHERE id = $1")
        .bind(body.message_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    broadcast_message_update(&state, conversation_id, body.message_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unpin_message(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let base = db::conversation_base(&state.pool, conversation_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))?;
    let me = db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    if base.kind == ConvKind::Group && !matches!(me.role, MemberRole::Owner | MemberRole::Admin) {
        return Err(AppError::forbidden("Only group admins can unpin messages."));
    }
    sqlx::query("UPDATE messages SET pinned_at = NULL WHERE conversation_id = $1 AND pinned_at IS NOT NULL")
        .bind(conversation_id)
        .execute(&state.pool)
        .await?;
    let pinned: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM messages WHERE conversation_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(conversation_id)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    if let Some(pid) = pinned {
        broadcast_message_update(&state, conversation_id, pid).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn mute(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    sqlx::query(
        "UPDATE conversation_members SET muted_at = now() WHERE conversation_id = $1 AND user_id = $2",
    )
    .bind(conversation_id)
    .bind(user.user_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unmute(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    db::require_membership(&state.pool, conversation_id, user.user_id).await?;
    sqlx::query(
        "UPDATE conversation_members SET muted_at = NULL WHERE conversation_id = $1 AND user_id = $2",
    )
    .bind(conversation_id)
    .bind(user.user_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Read markers
// ---------------------------------------------------------------------------

pub async fn mark_read(
    State(state): State<AppState>,
    user: AuthUser,
    Path(conversation_id): Path<Uuid>,
    Json(body): Json<MarkReadRequest>,
) -> ApiResult<StatusCode> {
    mark_read_core(&state, user.user_id, conversation_id, body.message_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Core read-marker logic, shared by the REST route and the WebSocket path.
pub async fn mark_read_core(
    state: &AppState,
    user_id: Uuid,
    conversation_id: Uuid,
    message_id: Uuid,
) -> ApiResult<()> {
    let me = db::require_membership(&state.pool, conversation_id, user_id).await?;
    let msg: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, conversation_id FROM messages WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(message_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((message_id, msg_conv)) = msg else {
        return Err(AppError::not_found("Message not found."));
    };
    if msg_conv != conversation_id {
        return Err(AppError::bad_request("That message is not in this chat."));
    }

    // Only advance the pointer forward.
    let current = me.last_read_message_id;
    let should_update = match current {
        Some(c) => message_id > c,
        None => true,
    };

    if should_update {
        let mut tx = state.pool.begin().await?;
        sqlx::query(
            "UPDATE conversation_members SET last_read_message_id = $1 WHERE conversation_id = $2 AND user_id = $3",
        )
        .bind(message_id)
        .bind(conversation_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO read_receipts (id, conversation_id, user_id, last_read_message_id)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (conversation_id, user_id)
             DO UPDATE SET last_read_message_id = $4, read_at = now()",
        )
        .bind(Uuid::now_v7())
        .bind(conversation_id)
        .bind(user_id)
        .bind(message_id)
        .execute(&mut *tx)
        .await?;
        // Related "message" notifications for this conversation are now read.
        sqlx::query(
            "UPDATE notifications SET read_at = now() WHERE user_id = $1 AND conversation_id = $2 AND kind = 'message' AND read_at IS NULL",
        )
        .bind(user_id)
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        // Broadcast my receipt, if my privacy settings allow it.
        let settings = db::fetch_settings(&state.pool, user_id).await?;
        if settings.read_receipts {
            let others = db::member_ids(&state.pool, conversation_id)
                .await?
                .into_iter()
                .filter(|u| *u != user_id)
                .collect::<Vec<_>>();
            state.hub.send_to_users(
                others,
                &ServerEvent::ReadReceipt {
                    conversation_id,
                    user_id,
                    last_read_message_id: message_id,
                },
            );
        }
    }

    Ok(())
}

/// Re-broadcast a message (edited/reaction/pin) to all conversation members.
pub(crate) async fn broadcast_message_update(
    state: &AppState,
    conversation_id: Uuid,
    message_id: Uuid,
) -> ApiResult<()> {
    let members = db::member_ids(&state.pool, conversation_id).await?;
    for uid in &members {
        if let Ok(msg) = db::build_message(&state.pool, message_id, *uid).await {
            state.hub.send_to_user(*uid, &ServerEvent::MessageUpdated { message: msg });
        }
    }
    Ok(())
}

/// Convenience used by tests.
pub async fn _unused(_: &AppState) -> ApiResult<()> {
    let _ = RATE_MESSAGE;
    Ok(())
}
