//! Typed HTTP client for the Nook backend.
//!
//! Everything the mobile app needs goes through this module: auth, profile,
//! friends, conversations, messages, media and notifications. The client keeps
//! the current bearer token behind a lock so the realtime socket and all
//! request paths share one source of truth.

use std::sync::RwLock;
use std::time::Duration;

use nook_shared::*;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use uuid::Uuid;

/// A user-facing API failure. `message` is always safe to show in the UI.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: Option<u16>,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn transport(message: impl Into<String>) -> Self {
        Self { status: None, code: "network".into(), message: message.into() }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == Some(401)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<reqwest::Error> for ApiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::transport("The server took too long to respond. Check your connection.")
        } else if err.is_connect() {
            Self::transport("Can't reach Nook right now. Is the server running?")
        } else {
            Self::transport("Network error. Please try again.")
        }
    }
}

/// Response of `GET /api/me`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MeResponse {
    pub user: UserPublic,
    pub settings: UserSettings,
    pub terms: TermsStatus,
}

/// Response of `GET /api/friend-requests`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RequestsResponse {
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

/// Response of `GET /api/conversations/{id}/messages`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct HistoryResponse {
    pub messages: Vec<Message>,
    pub has_more: bool,
}

/// Response of `GET /api/notifications`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct NotificationsResponse {
    pub notifications: Vec<Notification>,
    pub unread_count: u32,
    pub has_more: bool,
}

pub struct ApiClient {
    http: reqwest::Client,
    base: String,
    token: RwLock<Option<String>>,
}

impl ApiClient {
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");
        Self { http, base, token: RwLock::new(None) }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// WebSocket URL for the realtime connection.
    pub fn ws_url(&self) -> String {
        let mut url = self.base.clone();
        if let Some(rest) = url.strip_prefix("https://") {
            url = format!("wss://{rest}");
        } else if let Some(rest) = url.strip_prefix("http://") {
            url = format!("ws://{rest}");
        }
        format!("{url}/api/ws")
    }

    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().unwrap_or_else(|e| e.into_inner()) = token;
    }

    pub fn token(&self) -> Option<String> {
        self.token.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.token() {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn send_json<B: serde::Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<reqwest::Response, ApiError> {
        let request = self.auth(self.http.request(method, self.url(path))).json(body);
        Ok(request.send().await?)
    }

    async fn send_empty(&self, method: Method, path: &str) -> Result<reqwest::Response, ApiError> {
        let request = self.auth(self.http.request(method, self.url(path)));
        Ok(request.send().await?)
    }

    async fn parse<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, ApiError> {
        let status = response.status();
        let bytes = response.bytes().await?;
        if status.is_success() {
            serde_json::from_slice(&bytes).map_err(|e| {
                ApiError { status: Some(status.as_u16()), code: "decode".into(), message: format!("Unexpected server response ({e}).") }
            })
        } else {
            Err(Self::error_from(status, &bytes))
        }
    }

    fn error_from(status: StatusCode, bytes: &[u8]) -> ApiError {
        if let Ok(envelope) = serde_json::from_slice::<ApiErrorEnvelope>(bytes) {
            ApiError {
                status: Some(status.as_u16()),
                code: envelope.error.code,
                message: envelope.error.message,
            }
        } else {
            ApiError {
                status: Some(status.as_u16()),
                code: "http".into(),
                message: format!("Something went wrong (HTTP {}).", status.as_u16()),
            }
        }
    }

    async fn expect_ok(response: reqwest::Response) -> Result<(), ApiError> {
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let bytes = response.bytes().await.unwrap_or_default();
            Err(Self::error_from(status, &bytes))
        }
    }

    // -----------------------------------------------------------------------
    // Auth & onboarding
    // -----------------------------------------------------------------------

    pub async fn register(&self, body: &RegisterRequest) -> Result<AuthResponse, ApiError> {
        Self::parse(self.send_json(Method::POST, "/api/auth/register", body).await?).await
    }

    pub async fn login(&self, body: &LoginRequest) -> Result<AuthResponse, ApiError> {
        Self::parse(self.send_json(Method::POST, "/api/auth/login", body).await?).await
    }

    pub async fn recover(&self, body: &RecoverRequest) -> Result<AuthResponse, ApiError> {
        Self::parse(self.send_json(Method::POST, "/api/auth/recover", body).await?).await
    }

    pub async fn logout(&self) -> Result<(), ApiError> {
        Self::expect_ok(self.send_empty(Method::POST, "/api/auth/logout").await?).await
    }

    pub async fn me(&self) -> Result<MeResponse, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/me").await?).await
    }

    pub async fn accept_terms(&self, version: &str) -> Result<(), ApiError> {
        let body = AcceptTermsRequest { version: version.to_string() };
        Self::expect_ok(self.send_json(Method::POST, "/api/me/terms", &body).await?).await
    }

    pub async fn update_profile(&self, body: &UpdateProfileRequest) -> Result<UserPublic, ApiError> {
        Self::parse(self.send_json(Method::PATCH, "/api/me", body).await?).await
    }

    pub async fn change_password(&self, body: &ChangePasswordRequest) -> Result<(), ApiError> {
        Self::expect_ok(self.send_json(Method::PATCH, "/api/me/password", body).await?).await
    }

    pub async fn delete_account(&self, body: &DeleteAccountRequest) -> Result<(), ApiError> {
        // `DELETE` with a JSON body is supported by axum's Json extractor.
        let request = self.auth(self.http.request(Method::DELETE, self.url("/api/me"))).json(body);
        Self::expect_ok(request.send().await?).await
    }

    pub async fn regenerate_recovery_code(&self) -> Result<RecoveryCodeResponse, ApiError> {
        Self::parse(
            self.send_empty(Method::POST, "/api/me/recovery-codes/regenerate").await?,
        )
        .await
    }

    pub async fn update_settings(&self, body: &UpdateSettingsRequest) -> Result<UserSettings, ApiError> {
        Self::parse(self.send_json(Method::PATCH, "/api/me/settings", body).await?).await
    }

    // -----------------------------------------------------------------------
    // People
    // -----------------------------------------------------------------------

    pub async fn search(&self, query: &str) -> Result<SearchResponse, ApiError> {
        let request = self.auth(self.http.get(self.url("/api/users/search")).query(&[("q", query)]));
        Self::parse(request.send().await?).await
    }

    pub async fn friends(&self) -> Result<Vec<Friend>, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/friends").await?).await
    }

    pub async fn friend_requests(&self) -> Result<RequestsResponse, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/friend-requests").await?).await
    }

    pub async fn send_friend_request(&self, user_id: Uuid) -> Result<FriendRequest, ApiError> {
        let body = SendFriendRequestByUserId { user_id };
        Self::parse(self.send_json(Method::POST, "/api/friend-requests", &body).await?).await
    }

    pub async fn accept_friend_request(&self, request_id: Uuid) -> Result<FriendRequest, ApiError> {
        let path = format!("/api/friend-requests/{request_id}/accept");
        Self::parse(self.send_empty(Method::POST, &path).await?).await
    }

    pub async fn decline_friend_request(&self, request_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/friend-requests/{request_id}/decline");
        Self::expect_ok(self.send_empty(Method::POST, &path).await?).await
    }

    pub async fn remove_friend(&self, user_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/friends/{user_id}");
        Self::expect_ok(self.send_empty(Method::DELETE, &path).await?).await
    }

    pub async fn block(&self, user_id: Uuid) -> Result<(), ApiError> {
        let body = BlockRequest { user_id };
        Self::expect_ok(self.send_json(Method::POST, "/api/blocks", &body).await?).await
    }

    pub async fn unblock(&self, user_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/blocks/{user_id}");
        Self::expect_ok(self.send_empty(Method::DELETE, &path).await?).await
    }

    pub async fn blocks(&self) -> Result<Vec<UserPublic>, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/blocks").await?).await
    }

    pub async fn report(&self, body: &ReportRequest) -> Result<(), ApiError> {
        Self::expect_ok(self.send_json(Method::POST, "/api/reports", body).await?).await
    }

    // -----------------------------------------------------------------------
    // Conversations
    // -----------------------------------------------------------------------

    pub async fn conversations(&self) -> Result<Vec<ConversationSummary>, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/conversations").await?).await
    }

    pub async fn conversation(&self, id: Uuid) -> Result<ConversationDetail, ApiError> {
        let path = format!("/api/conversations/{id}");
        Self::parse(self.send_empty(Method::GET, &path).await?).await
    }

    pub async fn start_private(&self, user_id: Uuid) -> Result<ConversationSummary, ApiError> {
        let body = StartPrivateRequest { user_id };
        Self::parse(self.send_json(Method::POST, "/api/conversations/private", &body).await?).await
    }

    pub async fn create_group(&self, body: &CreateGroupRequest) -> Result<ConversationDetail, ApiError> {
        Self::parse(self.send_json(Method::POST, "/api/conversations/group", body).await?).await
    }

    pub async fn rename_group(&self, id: Uuid, title: &str) -> Result<ConversationDetail, ApiError> {
        let path = format!("/api/conversations/{id}");
        let form = reqwest::multipart::Form::new().text("title", title.to_string());
        let request = self.auth(self.http.patch(self.url(&path))).multipart(form);
        Self::parse(request.send().await?).await
    }

    pub async fn add_members(&self, id: Uuid, user_ids: Vec<Uuid>) -> Result<ConversationDetail, ApiError> {
        let path = format!("/api/conversations/{id}/members");
        let body = AddMembersRequest { user_ids };
        Self::parse(self.send_json(Method::POST, &path, &body).await?).await
    }

    pub async fn remove_member(&self, id: Uuid, user_id: Uuid) -> Result<ConversationDetail, ApiError> {
        let path = format!("/api/conversations/{id}/members/{user_id}");
        Self::parse(self.send_empty(Method::DELETE, &path).await?).await
    }

    pub async fn leave_group(&self, id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{id}/leave");
        Self::expect_ok(self.send_empty(Method::POST, &path).await?).await
    }

    pub async fn mute(&self, id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{id}/mute");
        Self::expect_ok(self.send_empty(Method::POST, &path).await?).await
    }

    pub async fn unmute(&self, id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{id}/unmute");
        Self::expect_ok(self.send_empty(Method::POST, &path).await?).await
    }

    pub async fn mark_read(&self, conversation_id: Uuid, message_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{conversation_id}/read");
        let body = MarkReadRequest { message_id };
        Self::expect_ok(self.send_json(Method::POST, &path, &body).await?).await
    }

    pub async fn pin(&self, conversation_id: Uuid, message_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{conversation_id}/pin");
        let body = MarkReadRequest { message_id };
        Self::expect_ok(self.send_json(Method::POST, &path, &body).await?).await
    }

    pub async fn unpin(&self, conversation_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/conversations/{conversation_id}/pin");
        Self::expect_ok(self.send_empty(Method::DELETE, &path).await?).await
    }

    // -----------------------------------------------------------------------
    // Messages
    // -----------------------------------------------------------------------

    pub async fn history(
        &self,
        conversation_id: Uuid,
        before_id: Option<Uuid>,
        limit: Option<i64>,
    ) -> Result<HistoryResponse, ApiError> {
        let path = format!("/api/conversations/{conversation_id}/messages");
        let mut request = self.auth(self.http.get(self.url(&path)));
        if let Some(before) = before_id {
            request = request.query(&[("before_id", before.to_string())]);
        }
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit.to_string())]);
        }
        Self::parse(request.send().await?).await
    }

    pub async fn send_message(
        &self,
        conversation_id: Uuid,
        body: &SendMessageRequest,
    ) -> Result<Message, ApiError> {
        let path = format!("/api/conversations/{conversation_id}/messages");
        Self::parse(self.send_json(Method::POST, &path, body).await?).await
    }

    pub async fn edit_message(&self, message_id: Uuid, body: &str) -> Result<Message, ApiError> {
        let path = format!("/api/messages/{message_id}");
        let body = EditMessageRequest { body: body.to_string() };
        Self::parse(self.send_json(Method::PATCH, &path, &body).await?).await
    }

    pub async fn delete_message(&self, message_id: Uuid, everywhere: bool) -> Result<(), ApiError> {
        let path = format!(
            "/api/messages/{message_id}?scope={}",
            if everywhere { "everyone" } else { "me" }
        );
        Self::expect_ok(self.send_empty(Method::DELETE, &path).await?).await
    }

    pub async fn set_reaction(&self, message_id: Uuid, emoji: &str) -> Result<Message, ApiError> {
        let path = format!("/api/messages/{message_id}/reactions");
        let body = SetReactionRequest { emoji: emoji.to_string() };
        Self::parse(self.send_json(Method::PUT, &path, &body).await?).await
    }

    pub async fn remove_reaction(&self, message_id: Uuid) -> Result<(), ApiError> {
        let path = format!("/api/messages/{message_id}/reactions");
        Self::expect_ok(self.send_empty(Method::DELETE, &path).await?).await
    }

    // -----------------------------------------------------------------------
    // Media
    // -----------------------------------------------------------------------

    pub async fn upload_image(
        &self,
        bytes: Vec<u8>,
        file_name: String,
        mime: String,
        purpose: &str,
    ) -> Result<AttachmentMeta, ApiError> {
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(file_name)
            .mime_str(&mime)
            .map_err(|e| ApiError::transport(format!("Unsupported image type ({e}).")))?;
        let form = reqwest::multipart::Form::new()
            .text("purpose", purpose.to_string())
            .part("file", part);
        let request = self.auth(self.http.post(self.url("/api/media/upload"))).multipart(form);
        let response: UploadImageResponse = Self::parse(request.send().await?).await?;
        Ok(response.attachment)
    }

    pub async fn download_media(&self, attachment_id: Uuid) -> Result<Vec<u8>, ApiError> {
        let path = format!("/api/media/{attachment_id}");
        let request = self.auth(self.http.get(self.url(&path)));
        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let bytes = response.bytes().await.unwrap_or_default();
            return Err(Self::error_from(status, &bytes));
        }
        Ok(response.bytes().await?.to_vec())
    }

    // -----------------------------------------------------------------------
    // Notifications
    // -----------------------------------------------------------------------

    pub async fn notifications(&self) -> Result<NotificationsResponse, ApiError> {
        Self::parse(self.send_empty(Method::GET, "/api/notifications").await?).await
    }

    pub async fn mark_notifications_read(&self, ids: Vec<Uuid>) -> Result<(), ApiError> {
        let body = MarkNotificationsReadRequest { ids };
        Self::expect_ok(self.send_json(Method::POST, "/api/notifications/read", &body).await?).await
    }

    pub async fn mark_all_notifications_read(&self) -> Result<(), ApiError> {
        Self::expect_ok(self.send_empty(Method::POST, "/api/notifications/read-all").await?).await
    }
}
