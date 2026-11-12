//! Router assembly and the authenticated-user extractor.

pub mod auth;
pub mod conversations;
pub mod friends;
pub mod me;
pub mod media;
pub mod messages;
pub mod notifications;
pub mod users;

use axum::extract::FromRequestParts;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use nook_shared::policies;

use crate::auth::AuthUser;
use crate::error::{AppError, ApiResult};
use crate::state::AppState;

/// Extractor that authenticates the request via `Authorization: Bearer <token>`.
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> ApiResult<Self> {
        crate::auth::authenticate(&state.pool, parts).await
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        // --- auth ---
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/recover", post(auth::recover))
        .route("/api/auth/logout", post(auth::logout))
        // --- me ---
        .route("/api/me", get(me::get_me).patch(me::update_me).delete(me::delete_account))
        .route("/api/me/avatar", patch(me::update_avatar))
        .route("/api/me/password", patch(me::change_password))
        .route(
            "/api/me/recovery-codes/regenerate",
            post(me::regenerate_recovery_code),
        )
        .route("/api/me/settings", get(me::get_settings).patch(me::update_settings))
        .route("/api/me/terms", get(me::terms_status).post(me::accept_terms))
        // --- policies (public, for the onboarding screen) ---
        .route("/api/policies", get(policies_handler))
        // --- users & search ---
        .route("/api/users/search", get(users::search))
        .route("/api/users/{user_id}", get(users::get_user))
        // --- friends ---
        .route(
            "/api/friend-requests",
            get(friends::list_requests).post(friends::send_request),
        )
        .route("/api/friend-requests/{request_id}/accept", post(friends::accept_request))
        .route("/api/friend-requests/{request_id}/decline", post(friends::decline_request))
        .route("/api/friend-requests/{request_id}", delete(friends::cancel_request))
        .route("/api/friends", get(friends::list_friends))
        .route("/api/friends/{user_id}", delete(friends::remove_friend))
        // --- blocks & reports ---
        .route("/api/blocks", get(friends::list_blocks).post(friends::block_user))
        .route("/api/blocks/{user_id}", delete(friends::unblock_user))
        .route("/api/reports", post(friends::report))
        // --- conversations ---
        .route("/api/conversations", get(conversations::list))
        .route("/api/conversations/private", post(conversations::start_private))
        .route("/api/conversations/group", post(conversations::create_group))
        .route(
            "/api/conversations/{conversation_id}",
            get(conversations::detail).patch(conversations::update_conversation),
        )
        .route(
            "/api/conversations/{conversation_id}/members",
            post(conversations::add_members),
        )
        .route(
            "/api/conversations/{conversation_id}/members/{user_id}",
            delete(conversations::remove_member),
        )
        .route("/api/conversations/{conversation_id}/leave", post(conversations::leave_group))
        .route(
            "/api/conversations/{conversation_id}/pin",
            post(conversations::pin_message).delete(conversations::unpin_message),
        )
        .route("/api/conversations/{conversation_id}/mute", post(conversations::mute))
        .route("/api/conversations/{conversation_id}/unmute", post(conversations::unmute))
        .route("/api/conversations/{conversation_id}/read", post(conversations::mark_read))
        .route(
            "/api/conversations/{conversation_id}/messages",
            get(messages::history).post(messages::send_message),
        )
        // --- messages ---
        .route(
            "/api/messages/{message_id}",
            get(messages::get_message)
                .patch(messages::edit_message)
                .delete(messages::delete_message),
        )
        .route(
            "/api/messages/{message_id}/reactions",
            put(messages::set_reaction).delete(messages::remove_reaction),
        )
        // --- media ---
        .route("/api/media/upload", post(media::upload))
        .route("/api/media/{attachment_id}", get(media::serve))
        // --- notifications ---
        .route("/api/notifications", get(notifications::list))
        .route("/api/notifications/read", post(notifications::mark_read))
        .route("/api/notifications/read-all", post(notifications::mark_all_read))
        // --- websocket ---
        .route("/api/ws", get(crate::ws::ws_handler))
        .with_state(state)
}

async fn policies_handler() -> impl axum::response::IntoResponse {
    #[derive(serde::Serialize)]
    struct Section {
        heading: String,
        body: String,
    }
    #[derive(serde::Serialize)]
    struct Doc {
        title: String,
        sections: Vec<Section>,
    }
    #[derive(serde::Serialize)]
    struct Payload {
        version: String,
        draft_notice: String,
        docs: Vec<Doc>,
    }

    let docs = policies::ALL_DOCS
        .iter()
        .map(|d| Doc {
            title: d.title.to_string(),
            sections: d
                .sections
                .iter()
                .map(|(h, b)| Section { heading: (*h).to_string(), body: (*b).to_string() })
                .collect(),
        })
        .collect();

    axum::Json(Payload {
        version: policies::POLICY_VERSION.to_string(),
        draft_notice: policies::DRAFT_NOTICE.to_string(),
        docs,
    })
}
