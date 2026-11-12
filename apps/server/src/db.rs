//! Shared database access helpers: row loading and assembly of the shared
//! DTOs (users, messages, conversations). All queries use runtime-checked
//! SQL so the build does not require a live database.

use chrono::{DateTime, Utc};
use nook_shared::*;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::{AppError, ApiResult};
use crate::state::{AppState, Hub};

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

pub async fn fetch_settings(pool: &PgPool, user_id: Uuid) -> ApiResult<UserSettings> {
    let row = sqlx::query(
        r#"
        SELECT online_visibility, last_seen_visibility, read_receipts, searchability,
               theme, notify_friend_requests, notify_messages, notify_group_events
        FROM user_settings WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let parse_vis = |s: &str| match s {
        "everyone" => Visibility::Everyone,
        "nobody" => Visibility::Nobody,
        _ => Visibility::Friends,
    };
    let parse_theme = |s: &str| match s {
        "light" => Theme::Light,
        "dark" => Theme::Dark,
        _ => Theme::System,
    };

    Ok(UserSettings {
        online_visibility: parse_vis(row.try_get::<String, _>(0)?.as_str()),
        last_seen_visibility: parse_vis(row.try_get::<String, _>(1)?.as_str()),
        read_receipts: row.try_get(2)?,
        searchability: parse_vis(row.try_get::<String, _>(3)?.as_str()),
        theme: parse_theme(row.try_get::<String, _>(4)?.as_str()),
        notify_friend_requests: row.try_get(5)?,
        notify_messages: row.try_get(6)?,
        notify_group_events: row.try_get(7)?,
    })
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

struct UserRow {
    id: Uuid,
    username: String,
    display_name: String,
    bio: String,
    avatar: Option<AttachmentMeta>,
}

type AvatarCols = (
    Option<Uuid>,
    Option<String>,
    Option<i64>,
    Option<i32>,
    Option<i32>,
);

fn avatar_meta(a: &AvatarCols) -> Option<AttachmentMeta> {
    let (id, mime, size, w, h) = a;
    let id = (*id)?;
    Some(AttachmentMeta {
        id,
        mime: mime.clone().unwrap_or_else(|| "image/jpeg".into()),
        byte_size: size.unwrap_or(0),
        width: w.map(|x| x as u32),
        height: h.map(|x| x as u32),
        url: format!("/api/media/{id}"),
    })
}

pub const USER_SELECT: &str = r#"
    u.id, u.username, u.display_name, u.bio,
    a.id AS av_id, a.mime AS av_mime, a.byte_size AS av_size,
    a.width AS av_width, a.height AS av_height
"#;

fn user_from_row(row: &sqlx::postgres::PgRow) -> Result<UserRow, sqlx::Error> {
    let avatar: AvatarCols = (
        row.try_get("av_id")?,
        row.try_get("av_mime")?,
        row.try_get("av_size")?,
        row.try_get("av_width")?,
        row.try_get("av_height")?,
    );
    Ok(UserRow {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        display_name: row.try_get("display_name")?,
        bio: row.try_get("bio")?,
        avatar: avatar_meta(&avatar),
    })
}

pub async fn relationship(pool: &PgPool, viewer: Uuid, other: Uuid) -> ApiResult<Relationship> {
    if viewer == other {
        return Ok(Relationship::None);
    }
    let blocked: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM blocked_users WHERE (blocker_id = $1 AND blocked_id = $2))",
    )
    .bind(viewer)
    .bind(other)
    .fetch_one(pool)
    .await?;
    if blocked {
        return Ok(Relationship::BlockedByMe);
    }
    let friends: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE (user_id_a, user_id_b) = (least($1,$2), greatest($1,$2)))",
    )
    .bind(viewer)
    .bind(other)
    .fetch_one(pool)
    .await?;
    if friends {
        return Ok(Relationship::Friends);
    }
    let sent: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friend_requests WHERE sender_id = $1 AND receiver_id = $2 AND status = 'pending')",
    )
    .bind(viewer)
    .bind(other)
    .fetch_one(pool)
    .await?;
    if sent {
        return Ok(Relationship::RequestSent);
    }
    let received: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friend_requests WHERE sender_id = $2 AND receiver_id = $1 AND status = 'pending')",
    )
    .bind(viewer)
    .bind(other)
    .fetch_one(pool)
    .await?;
    if received {
        return Ok(Relationship::RequestReceived);
    }
    Ok(Relationship::None)
}

/// Loads a user as `viewer` sees them (relationship computed when viewer given).
pub async fn user_public(
    pool: &PgPool,
    user_id: Uuid,
    viewer: Option<Uuid>,
) -> ApiResult<UserPublic> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {USER_SELECT} FROM users u LEFT JOIN attachments a ON a.id = u.avatar_attachment_id WHERE u.id = $1"
    )))
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("That person doesn't exist (or left Nook)."))?;
    let u = user_from_row(&row)?;
    let relationship = match viewer {
        Some(v) => Some(relationship(pool, v, user_id).await?),
        None => None,
    };
    Ok(UserPublic {
        id: u.id,
        username: u.username,
        display_name: u.display_name,
        bio: u.bio,
        avatar: u.avatar,
        relationship,
    })
}

/// Public profile of the acting user (relationship always None).
pub async fn me_public(state: &AppState, user_id: Uuid) -> ApiResult<UserPublic> {
    user_public(&state.pool, user_id, None).await
}

pub async fn user_exists(pool: &PgPool, user_id: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(pool)
        .await?)
}

// ---------------------------------------------------------------------------
// Social helpers
// ---------------------------------------------------------------------------

pub async fn are_friends(pool: &PgPool, a: Uuid, b: Uuid) -> ApiResult<bool> {
    if a == b {
        return Ok(false);
    }
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE (user_id_a, user_id_b) = (least($1,$2), greatest($1,$2)))",
    )
    .bind(a)
    .bind(b)
    .fetch_one(pool)
    .await?)
}

pub async fn is_blocked(pool: &PgPool, blocker: Uuid, blocked: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM blocked_users WHERE blocker_id = $1 AND blocked_id = $2)",
    )
    .bind(blocker)
    .bind(blocked)
    .fetch_one(pool)
    .await?)
}

pub async fn blocked_either(pool: &PgPool, a: Uuid, b: Uuid) -> ApiResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM blocked_users WHERE (blocker_id = $1 AND blocked_id = $2) OR (blocker_id = $2 AND blocked_id = $1))",
    )
    .bind(a)
    .bind(b)
    .fetch_one(pool)
    .await?)
}

// ---------------------------------------------------------------------------
// Conversations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConvKind {
    Private,
    Group,
}

pub fn parse_kind(s: &str) -> ConvKind {
    match s {
        "group" => ConvKind::Group,
        _ => ConvKind::Private,
    }
}

pub fn parse_role(s: &str) -> MemberRole {
    match s {
        "owner" => MemberRole::Owner,
        "admin" => MemberRole::Admin,
        _ => MemberRole::Member,
    }
}

pub struct MemberRow {
    pub conversation_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
    pub muted_at: Option<DateTime<Utc>>,
    pub pinned_at: Option<DateTime<Utc>>,
    pub last_read_message_id: Option<Uuid>,
    pub joined_at: DateTime<Utc>,
}

pub async fn membership(pool: &PgPool, conversation_id: Uuid, user_id: Uuid) -> ApiResult<Option<MemberRow>> {
    let row = sqlx::query(
        r#"
        SELECT conversation_id, user_id, role, muted_at, pinned_at, last_read_message_id, joined_at
        FROM conversation_members
        WHERE conversation_id = $1 AND user_id = $2
        "#,
    )
    .bind(conversation_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(MemberRow {
        conversation_id: row.try_get(0)?,
        user_id: row.try_get(1)?,
        role: parse_role(&row.try_get::<String, _>(2)?),
        muted_at: row.try_get(3)?,
        pinned_at: row.try_get(4)?,
        last_read_message_id: row.try_get(5)?,
        joined_at: row.try_get(6)?,
    }))
}

pub async fn require_membership(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
) -> ApiResult<MemberRow> {
    membership(pool, conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation not found."))
}

pub async fn member_ids(pool: &PgPool, conversation_id: Uuid) -> ApiResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?)
}

pub async fn member_count(pool: &PgPool, conversation_id: Uuid) -> ApiResult<u32> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM conversation_members WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_one(pool)
    .await? as u32)
}

pub struct ConvBase {
    pub id: Uuid,
    pub kind: ConvKind,
    pub title: Option<String>,
    pub description: String,
    pub avatar: Option<AttachmentMeta>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_message_at: DateTime<Utc>,
}

pub async fn conversation_base(pool: &PgPool, conversation_id: Uuid) -> ApiResult<Option<ConvBase>> {
    let row = sqlx::query(
        r#"
        SELECT c.id, c.kind, c.title, c.description,
               a.id, a.mime, a.byte_size, a.width, a.height,
               c.created_by, c.created_at, c.last_message_at
        FROM conversations c
        LEFT JOIN attachments a ON a.id = c.avatar_attachment_id
        WHERE c.id = $1
        "#,
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let avatar: AvatarCols = (
        row.try_get(4)?,
        row.try_get(5)?,
        row.try_get(6)?,
        row.try_get(7)?,
        row.try_get(8)?,
    );
    Ok(Some(ConvBase {
        id: row.try_get(0)?,
        kind: parse_kind(&row.try_get::<String, _>(1)?),
        title: row.try_get(2)?,
        description: row.try_get(3)?,
        avatar: avatar_meta(&avatar),
        created_by: row.try_get(9)?,
        created_at: row.try_get(10)?,
        last_message_at: row.try_get(11)?,
    }))
}

/// Assembles a `ConversationSummary` for one (conversation, viewer) pair.
/// `other` is the other participant for private chats.
pub async fn conversation_summary(
    state: &AppState,
    base: &ConvBase,
    member: &MemberRow,
    viewer: Uuid,
) -> ApiResult<ConversationSummary> {
    let kind = base.kind;
    let (other_user, title, other_online) = match kind {
        ConvKind::Private => {
            let other_id = private_other_id(&state.pool, base.id, viewer)
                .await?
                .ok_or_else(|| AppError::internal("private conversation missing participant"))?;
            let other = user_public(&state.pool, other_id, Some(viewer)).await?;
            let online = presence_visible_to(&state.pool, &state.hub, other_id, viewer).await;
            let title = other.display_name.clone();
            (Some(other), title, Some(online))
        }
        ConvKind::Group => (None, base.title.clone().unwrap_or_default(), None),
    };

    let last_message = last_message_for(state, base.id, viewer).await?;
    let unread_count = unread_count(&state.pool, base.id, viewer, member.last_read_message_id).await?;

    Ok(ConversationSummary {
        id: base.id,
        kind: match kind {
            ConvKind::Private => ConversationKind::Private,
            ConvKind::Group => ConversationKind::Group,
        },
        title,
        description: if base.description.is_empty() { None } else { Some(base.description.clone()) },
        avatar: base.avatar.clone(),
        other_user,
        last_message,
        unread_count,
        pinned: member.pinned_at.is_some(),
        muted: member.muted_at.is_some(),
        member_count: member_count(&state.pool, base.id).await?,
        other_online,
        updated_at: base.last_message_at,
    })
}

pub async fn private_other_id(pool: &PgPool, conversation_id: Uuid, viewer: Uuid) -> ApiResult<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT user_id FROM conversation_members WHERE conversation_id = $1 AND user_id <> $2 LIMIT 1",
    )
    .bind(conversation_id)
    .bind(viewer)
    .fetch_optional(pool)
    .await?)
}

/// Whether `subject`'s presence is visible to `viewer`.
pub async fn presence_visible_to(pool: &PgPool, hub: &Hub, subject: Uuid, viewer: Uuid) -> bool {
    if subject == viewer {
        return hub.is_online(subject);
    }
    let online = hub.is_online(subject);
    let Ok(settings) = fetch_settings(pool, subject).await else {
        return false;
    };
    let allowed = match settings.online_visibility {
        Visibility::Everyone => true,
        Visibility::Friends => are_friends(pool, subject, viewer).await.unwrap_or(false),
        Visibility::Nobody => false,
    };
    allowed && online
}

pub async fn last_message_for(state: &AppState, conversation_id: Uuid, viewer: Uuid) -> ApiResult<Option<Message>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT m.id
        FROM messages m
        WHERE m.conversation_id = $1
          AND m.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM message_hides h WHERE h.user_id = $2 AND h.message_id = m.id)
        ORDER BY m.id DESC
        LIMIT 1
        "#,
    )
    .bind(conversation_id)
    .bind(viewer)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    match id {
        Some(mid) => Ok(Some(build_message(&state.pool, mid, viewer).await?)),
        None => Ok(None),
    }
}

pub async fn unread_count(
    pool: &PgPool,
    conversation_id: Uuid,
    viewer: Uuid,
    last_read: Option<Uuid>,
) -> ApiResult<u32> {
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT count(*)
        FROM messages m
        WHERE m.conversation_id = $1
          AND m.sender_id <> $2
          AND m.deleted_at IS NULL
          AND m.kind <> 'system'
          AND NOT EXISTS (SELECT 1 FROM message_hides h WHERE h.user_id = $2 AND h.message_id = m.id)
          AND ($3::uuid IS NULL OR m.id > $3)
        "#,
    )
    .bind(conversation_id)
    .bind(viewer)
    .bind(last_read)
    .fetch_one(pool)
    .await?;
    Ok(count as u32)
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

pub struct MessageRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender: UserPublic,
    pub kind: MessageKind,
    pub body: String,
    pub attachment: Option<AttachmentMeta>,
    pub reply_to_id: Option<Uuid>,
    pub pinned_at: Option<DateTime<Utc>>,
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn load_message_row(
    pool: &PgPool,
    message_id: Uuid,
    viewer: Uuid,
) -> ApiResult<Option<MessageRow>> {
    let row = sqlx::query(
        r#"
        SELECT
            m.id, m.conversation_id, m.kind, m.body, m.reply_to_id,
            m.pinned_at, m.edited_at, m.created_at,
            s.id, s.username, s.display_name, s.bio,
            sa.id, sa.mime, sa.byte_size, sa.width, sa.height,
            at.id, at.mime, at.byte_size, at.width, at.height
        FROM messages m
        JOIN users s ON s.id = m.sender_id
        LEFT JOIN attachments sa ON sa.id = s.avatar_attachment_id
        LEFT JOIN attachments at ON at.id = m.attachment_id
        WHERE m.id = $1
          AND m.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM message_hides h WHERE h.user_id = $2 AND h.message_id = m.id)
        "#,
    )
    .bind(message_id)
    .bind(viewer)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let sender_avatar: AvatarCols = (
        row.try_get(12)?,
        row.try_get(13)?,
        row.try_get(14)?,
        row.try_get(15)?,
        row.try_get(16)?,
    );
    let attachment: AvatarCols = (
        row.try_get(17)?,
        row.try_get(18)?,
        row.try_get(19)?,
        row.try_get(20)?,
        row.try_get(21)?,
    );
    Ok(Some(MessageRow {
        id: row.try_get(0)?,
        conversation_id: row.try_get(1)?,
        kind: parse_message_kind(&row.try_get::<String, _>(2)?),
        body: row.try_get(3)?,
        reply_to_id: row.try_get(4)?,
        pinned_at: row.try_get(5)?,
        edited_at: row.try_get(6)?,
        created_at: row.try_get(7)?,
        sender: UserPublic {
            id: row.try_get(8)?,
            username: row.try_get(9)?,
            display_name: row.try_get(10)?,
            bio: row.try_get(11)?,
            avatar: avatar_meta(&sender_avatar),
            relationship: None,
        },
        attachment: avatar_meta(&attachment),
    }))
}

pub fn parse_message_kind(s: &str) -> MessageKind {
    match s {
        "image" => MessageKind::Image,
        "system" => MessageKind::System,
        _ => MessageKind::Text,
    }
}

pub async fn message_ids_page(
    pool: &PgPool,
    conversation_id: Uuid,
    viewer: Uuid,
    before: Option<Uuid>,
    limit: i64,
) -> ApiResult<Vec<Uuid>> {
    let rows: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT m.id
        FROM messages m
        WHERE m.conversation_id = $1
          AND m.deleted_at IS NULL
          AND ($2::uuid IS NULL OR m.id < $2)
          AND NOT EXISTS (SELECT 1 FROM message_hides h WHERE h.user_id = $3 AND h.message_id = m.id)
        ORDER BY m.id DESC
        LIMIT $4
        "#,
    )
    .bind(conversation_id)
    .bind(before)
    .bind(viewer)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Builds the full shared `Message` DTO including reply preview and reactions.
pub async fn build_message(pool: &PgPool, message_id: Uuid, viewer: Uuid) -> ApiResult<Message> {
    let row = load_message_row(pool, message_id, viewer)
        .await?
        .ok_or_else(|| AppError::not_found("Message not found."))?;
    Ok(assemble(pool, row, viewer).await?)
}

pub async fn assemble(pool: &PgPool, row: MessageRow, viewer: Uuid) -> ApiResult<Message> {
    let reply_to = match row.reply_to_id {
        Some(rid) => load_reply_preview(pool, rid).await?,
        None => None,
    };

    let mut reactions: Vec<ReactionGroup> = Vec::new();
    let reaction_rows = sqlx::query(
        r#"
        SELECT r.emoji, r.user_id, u.username, u.display_name, u.bio,
               ua.id, ua.mime, ua.byte_size, ua.width, ua.height
        FROM message_reactions r
        JOIN users u ON u.id = r.user_id
        LEFT JOIN attachments ua ON ua.id = u.avatar_attachment_id
        WHERE r.message_id = $1
        ORDER BY r.created_at
        "#,
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;

    for rr in &reaction_rows {
        let emoji: String = rr.try_get(0)?;
        let reactor = UserPublic {
            id: rr.try_get(1)?,
            username: rr.try_get(2)?,
            display_name: rr.try_get(3)?,
            bio: rr.try_get(4)?,
            avatar: avatar_meta(&(
                rr.try_get(5)?,
                rr.try_get(6)?,
                rr.try_get(7)?,
                rr.try_get(8)?,
                rr.try_get(9)?,
            )),
            relationship: None,
        };
        if let Some(group) = reactions.iter_mut().find(|g| g.emoji == emoji) {
            group.count += 1;
            group.me |= reactor.id == viewer;
            if group.reactors.len() < 5 {
                group.reactors.push(reactor);
            }
        } else {
            let me = reactor.id == viewer;
            reactions.push(ReactionGroup {
                emoji,
                count: 1,
                me,
                reactors: vec![reactor],
            });
        }
    }

    Ok(Message {
        id: row.id,
        conversation_id: row.conversation_id,
        sender: row.sender,
        kind: row.kind,
        body: row.body,
        attachment: row.attachment,
        reply_to: reply_to.map(Box::new),
        reactions,
        pinned: row.pinned_at.is_some(),
        edited: row.edited_at.is_some(),
        created_at: row.created_at,
        edited_at: row.edited_at,
    })
}

pub async fn load_reply_preview(pool: &PgPool, message_id: Uuid) -> ApiResult<Option<ReplyPreview>> {
    let row = sqlx::query(
        r#"
        SELECT m.id, m.kind, m.body,
               s.id, s.username, s.display_name, s.bio,
               sa.id, sa.mime, sa.byte_size, sa.width, sa.height
        FROM messages m
        JOIN users s ON s.id = m.sender_id
        LEFT JOIN attachments sa ON sa.id = s.avatar_attachment_id
        WHERE m.id = $1 AND m.deleted_at IS NULL
        "#,
    )
    .bind(message_id)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(ReplyPreview {
        id: row.try_get(0)?,
        kind: parse_message_kind(&row.try_get::<String, _>(1)?),
        body: row.try_get(2)?,
        sender: UserPublic {
            id: row.try_get(3)?,
            username: row.try_get(4)?,
            display_name: row.try_get(5)?,
            bio: row.try_get(6)?,
            avatar: avatar_meta(&(
                row.try_get(7)?,
                row.try_get(8)?,
                row.try_get(9)?,
                row.try_get(10)?,
                row.try_get(11)?,
            )),
            relationship: None,
        },
    }))
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

pub async fn notification_to_dto(pool: &PgPool, row_id: Uuid, viewer: Uuid) -> ApiResult<Notification> {
    let row = sqlx::query(
        r#"
        SELECT n.id, n.kind, n.title, n.body, n.conversation_id, n.message_id,
               n.data, n.read_at, n.created_at,
               a.id, a.username, a.display_name, a.bio,
               aa.id, aa.mime, aa.byte_size, aa.width, aa.height
        FROM notifications n
        LEFT JOIN users a ON a.id = n.actor_id
        LEFT JOIN attachments aa ON aa.id = a.avatar_attachment_id
        WHERE n.id = $1
        "#,
    )
    .bind(row_id)
    .fetch_one(pool)
    .await?;

    let actor: Option<UserPublic> = if row.try_get::<Option<Uuid>, _>(9)?.is_some() {
        Some(UserPublic {
            id: row.try_get(9)?,
            username: row.try_get(10)?,
            display_name: row.try_get(11)?,
            bio: row.try_get(12)?,
            avatar: avatar_meta(&(
                row.try_get(14)?,
                row.try_get(15)?,
                row.try_get(16)?,
                row.try_get(17)?,
                row.try_get(18)?,
            )),
            relationship: None,
        })
    } else {
        None
    };

    Ok(Notification {
        id: row.try_get(0)?,
        kind: parse_notification_kind(&row.try_get::<String, _>(1)?),
        title: row.try_get(2)?,
        body: row.try_get(3)?,
        actor,
        conversation_id: row.try_get(4)?,
        message_id: row.try_get(5)?,
        data: row.try_get(6)?,
        read: row.try_get::<Option<DateTime<Utc>>, _>(7)?.is_some(),
        created_at: row.try_get(8)?,
    })
}

pub fn parse_notification_kind(s: &str) -> NotificationKind {
    match s {
        "friend_request_received" => NotificationKind::FriendRequestReceived,
        "friend_request_accepted" => NotificationKind::FriendRequestAccepted,
        "added_to_group" => NotificationKind::AddedToGroup,
        "removed_from_group" => NotificationKind::RemovedFromGroup,
        "group_updated" => NotificationKind::GroupUpdated,
        "member_joined" => NotificationKind::MemberJoined,
        "member_left" => NotificationKind::MemberLeft,
        _ => NotificationKind::Message,
    }
}

/// Creates a notification + pushes a WS event, honoring per-user prefs.
/// `kind_db` is the raw DB kind; `notify_when` gates on the user's settings.
pub async fn notify(
    state: &AppState,
    user_id: Uuid,
    actor_id: Option<Uuid>,
    kind_db: &'static str,
    title: String,
    body: String,
    conversation_id: Option<Uuid>,
    message_id: Option<Uuid>,
    data: serde_json::Value,
    respect_pref: Option<bool>,
) -> ApiResult<Option<Uuid>> {
    if let Some(pref) = respect_pref {
        if !pref {
            return Ok(None);
        }
    }
    let id = Uuid::now_v7();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO notifications (id, user_id, kind, title, body, actor_id, conversation_id, message_id, data)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(kind_db)
    .bind(&title)
    .bind(&body)
    .bind(actor_id)
    .bind(conversation_id)
    .bind(message_id)
    .bind(&data)
    .fetch_one(&state.pool)
    .await?;

    let dto = notification_to_dto(&state.pool, inserted, user_id).await?;
    state.hub.send_to_user(user_id, &ServerEvent::Notification { notification: dto });
    Ok(Some(inserted))
}
