//! WebSocket protocol for real-time events.
//!
//! One WebSocket connection per authenticated user carries all events:
//! messages, typing, read receipts, presence, notifications and
//! conversation/group updates. Messages are JSON-encoded `ClientEvent`
//! (client → server) and `ServerEvent` (server → client).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{FriendRequest, Message, Notification, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEvent {
    /// Keep-alive; server answers with `Pong`.
    Ping,
    /// The user is typing in the given conversation. Sent sparsely.
    Typing { conversation_id: Uuid },
    /// The user has read up to and including `message_id`.
    Read { conversation_id: Uuid, message_id: Uuid },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// Sent once after the socket is accepted. `online_user_ids` lists
    /// currently connected friends.
    Ready {
        user_id: Uuid,
        online_user_ids: Vec<Uuid>,
    },
    Pong,
    /// A new message was persisted in a conversation the user belongs to.
    MessageNew { message: Message },
    /// A message was edited, reacted to, pinned or unpinned.
    MessageUpdated { message: Message },
    /// A message was deleted for everyone (tombstone).
    MessageDeleted { conversation_id: Uuid, message_id: Uuid },
    /// Someone is typing in a conversation.
    Typing {
        conversation_id: Uuid,
        user_id: Uuid,
        display_name: String,
    },
    /// A member's read pointer advanced.
    ReadReceipt {
        conversation_id: Uuid,
        user_id: Uuid,
        last_read_message_id: Uuid,
    },
    /// A friend came online or went offline.
    Presence {
        user_id: Uuid,
        online: bool,
        last_seen_at: Option<Timestamp>,
    },
    /// A notification was created for this user.
    Notification { notification: Notification },
    /// A conversation this user belongs to changed (list refresh hint).
    ConversationUpdated { conversation: ConversationSummaryLite },
    /// A friend request was created for (or by) this user.
    FriendRequest { request: FriendRequest },
    /// An outgoing or incoming friend request was accepted.
    FriendRequestAccepted { request: FriendRequest, conversation_id: Uuid },
    /// Friend request declined or cancelled.
    FriendRequestClosed { request_id: Uuid },
    /// A user was added to a group.
    MemberAdded { conversation_id: Uuid },
    /// A user was removed from (or left) a group.
    MemberRemoved { conversation_id: Uuid, user_id: Uuid },
    /// Unrecoverable protocol error; connection will be closed after.
    Error { message: String },
}

/// Compact summary used in `ConversationUpdated` events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummaryLite {
    pub id: Uuid,
    pub kind: crate::models::ConversationKind,
    pub title: String,
    pub member_count: u32,
    pub updated_at: Timestamp,
}
