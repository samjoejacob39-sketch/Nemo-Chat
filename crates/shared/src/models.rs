//! Data transfer objects shared between server and mobile client.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Serialized timestamps use RFC 3339 strings in UTC.
pub type Timestamp = chrono::DateTime<chrono::Utc>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Private,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    Image,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    None,
    Friends,
    RequestSent,
    RequestReceived,
    BlockedByMe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Everyone,
    Friends,
    Nobody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendRequestStatus {
    Pending,
    Accepted,
    Declined,
    Cancelled,
}

/// Describes an uploaded image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub mime: String,
    pub byte_size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Relative URL under the API base, e.g. `/api/media/<id>`.
    pub url: String,
}

/// Publicly visible user information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub avatar: Option<AttachmentMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<Relationship>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReactionGroup {
    pub emoji: String,
    pub count: u32,
    /// Whether the current user has reacted with this emoji.
    pub me: bool,
    /// Up to five reactors, for avatars next to the reaction pill.
    pub reactors: Vec<UserPublic>,
}

/// A chat message as returned by the API and delivered over WebSocket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub sender: UserPublic,
    pub kind: MessageKind,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<AttachmentMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Box<ReplyPreview>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub reactions: Vec<ReactionGroup>,
    pub pinned: bool,
    pub edited: bool,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplyPreview {
    pub id: Uuid,
    pub sender: UserPublic,
    pub kind: MessageKind,
    pub body: String,
}

/// Last activity of a conversation, as shown in the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub kind: ConversationKind,
    /// For private chats: the other user's display name; for groups the title.
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<AttachmentMeta>,
    /// For private chats: the other participant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_user: Option<UserPublic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<Message>,
    pub unread_count: u32,
    pub pinned: bool,
    pub muted: bool,
    pub member_count: u32,
    /// Presence of the other user (private chats, respecting their privacy).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_online: Option<bool>,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationMember {
    pub user: UserPublic,
    pub role: MemberRole,
    pub joined_at: Timestamp,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationDetail {
    pub summary: ConversationSummary,
    pub members: Vec<ConversationMember>,
    pub my_role: MemberRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_message: Option<Message>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: Uuid,
    pub sender: UserPublic,
    pub receiver: UserPublic,
    pub status: FriendRequestStatus,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Friend {
    pub user: UserPublic,
    pub since: Timestamp,
    pub conversation_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    FriendRequestReceived,
    FriendRequestAccepted,
    AddedToGroup,
    RemovedFromGroup,
    GroupUpdated,
    MemberJoined,
    MemberLeft,
    /// A new message arrived while the user was offline.
    Message,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<UserPublic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
    pub read: bool,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    pub online_visibility: Visibility,
    pub last_seen_visibility: Visibility,
    pub read_receipts: bool,
    pub searchability: Visibility,
    pub theme: Theme,
    pub notify_friend_requests: bool,
    pub notify_messages: bool,
    pub notify_group_events: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            online_visibility: Visibility::Friends,
            last_seen_visibility: Visibility::Friends,
            read_receipts: true,
            searchability: Visibility::Everyone,
            theme: Theme::System,
            notify_friend_requests: true,
            notify_messages: true,
            notify_group_events: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Auth & onboarding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Opaque session token. Must be kept secret.
    pub token: String,
    pub user: UserPublic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub display_name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoverRequest {
    pub username: String,
    pub recovery_code: String,
    pub new_password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthResponse {
    pub session: SessionInfo,
    /// True when the user must accept the current policy version.
    pub onboarding_required: bool,
    /// Only present on registration, shown exactly once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TermsStatus {
    pub current_version: String,
    pub latest_accepted_version: Option<String>,
    pub onboarding_required: bool,
    pub accepted_at: Option<Timestamp>,
}

// ---------------------------------------------------------------------------
// Request payloads for the REST API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteAccountRequest {
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartPrivateRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub member_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_attachment_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddMembersRequest {
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateConversationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_attachment_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub kind: MessageKind,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditMessageRequest {
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteScope {
    Me,
    Everyone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetReactionRequest {
    pub emoji: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkReadRequest {
    pub message_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SendFriendRequestByUserId {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    pub reason: String,
    #[serde(default)]
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_visibility: Option<Visibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_visibility: Option<Visibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_receipts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub searchability: Option<Visibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_friend_requests: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_messages: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_group_events: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcceptTermsRequest {
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkNotificationsReadRequest {
    pub ids: Vec<Uuid>,
}

/// Response of `GET /api/users/search`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<UserPublic>,
    pub query: String,
}

/// Structured API error bodies: `{ "error": { "code": "...", "message": "..." } }`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorEnvelope {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UploadImageResponse {
    pub attachment: AttachmentMeta,
}

/// Response of `POST /api/me/recovery-codes/regenerate`.
/// The new code is shown exactly once and never returned again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryCodeResponse {
    pub code: String,
}

/// Sanity check so consumers can trust the emoji palette shipped with the UI.
#[test]
fn reaction_palette_is_valid() {
    use crate::validation::REACTION_EMOJIS;
    assert!(!REACTION_EMOJIS.is_empty());
    for e in REACTION_EMOJIS {
        assert!(!e.trim().is_empty());
    }
}
