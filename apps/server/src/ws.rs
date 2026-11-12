//! WebSocket endpoint: real-time messages, typing, read receipts,
//! presence and notifications. One connection per signed-in device.

use std::collections::HashSet;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::header::AUTHORIZATION;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use nook_shared::{ClientEvent, ServerEvent, Visibility};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth;
use crate::db;
use crate::error::AppError;
use crate::state::{AppState, RATE_WS};

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub async fn ws_handler(
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
    headers: axum::http::HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Prefer the Authorization header; fall back to ?token= for clients
    // that cannot set headers.
    let header_token = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string());
    let token = header_token.or(q.token);

    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return AppError::unauthorized().into_response();
    };

    let auth_user = match auth::authenticate_token(&state.pool, &token).await {
        Ok(a) => a,
        Err(_) => return AppError::unauthorized().into_response(),
    };

    let user_id = auth_user.user_id;
    tracing::debug!(%user_id, "websocket connect");

    upgrade.on_upgrade(move |socket| async move {
        let _ = handle_socket(state, user_id, socket).await;
    })
}

async fn handle_socket(state: AppState, user_id: Uuid, socket: WebSocket) -> Result<(), ()> {
    let was_online_before = state.hub.is_online(user_id);
    let (handle, mut hub_rx) = state.hub.register(user_id);
    let first_connection = !was_online_before;

    // Presence: notify friends when I come online (respecting my setting).
    if first_connection {
        broadcast_presence(&state, user_id, true, None).await;
    }

    let (mut ws_tx, mut ws_rx) = socket.split();
    let (inject_tx, mut inject_rx) = tokio::sync::mpsc::unbounded_channel::<ServerEvent>();

    // Ready: my id + which of my friends are currently online.
    let friend_ids = friend_ids(&state, user_id).await.unwrap_or_default();
    let mut online_friends: Vec<Uuid> = Vec::new();
    for f in &friend_ids {
        if state.hub.is_online(*f) {
            let visible = match db::fetch_settings(&state.pool, *f)
                .await
                .map(|s| s.online_visibility)
                .unwrap_or(Visibility::Friends)
            {
                Visibility::Nobody => false,
                Visibility::Friends => true, // we are friends
                Visibility::Everyone => true,
            };
            if visible {
                online_friends.push(*f);
            }
        }
    }
    let _ = inject_tx.send(ServerEvent::Ready { user_id, online_user_ids: online_friends });

    // Forwarder: hub events + injected events → socket.
    let forwarder = tokio::spawn(async move {
        loop {
            tokio::select! {
                ev = hub_rx.recv() => {
                    match ev {
                        Some(ev) => {
                            let Ok(text) = serde_json::to_string(&ev) else { continue };
                            if ws_tx.send(WsMessage::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                ev = inject_rx.recv() => {
                    match ev {
                        Some(ev) => {
                            let Ok(text) = serde_json::to_string(&ev) else { continue };
                            if ws_tx.send(WsMessage::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    // Read loop: client events.
    let mut result = Ok(());
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(WsMessage::Text(text)) => {
                let event: ClientEvent = match serde_json::from_str(text.as_str()) {
                    Ok(e) => e,
                    Err(_) => {
                        let _ = inject_tx.send(ServerEvent::Error {
                            message: "Unrecognized message format.".into(),
                        });
                        continue;
                    }
                };
                if state.limiter.check(&user_id.to_string(), &RATE_WS).is_err() {
                    continue; // silently drop floods
                }
                match event {
                    ClientEvent::Ping => {
                        let _ = inject_tx.send(ServerEvent::Pong);
                    }
                    ClientEvent::Typing { conversation_id } => {
                        if let Ok(me) = db::require_membership(&state.pool, conversation_id, user_id).await {
                            let _ = me;
                            if let Ok(members) = db::member_ids(&state.pool, conversation_id).await {
                                let display_name = crate::routes::conversations::display_name_of(&state, user_id)
                                    .await
                                    .unwrap_or_default();
                                for m in members {
                                    if m != user_id {
                                        state.hub.send_to_user(
                                            m,
                                            &ServerEvent::Typing {
                                                conversation_id,
                                                user_id,
                                                display_name: display_name.clone(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ClientEvent::Read { conversation_id, message_id } => {
                        let _ = crate::routes::conversations::mark_read_core(
                            &state, user_id, conversation_id, message_id,
                        )
                        .await;
                    }
                }
            }
            // tungstenite/axum auto-respond to protocol-level pings,
            // so we never need the split sink here.
            Ok(WsMessage::Ping(_)) => {}
            Ok(WsMessage::Close(_)) => break,
            Ok(_) => {}
            Err(_) => {
                result = Err(());
                break;
            }
        }
    }

    forwarder.abort();
    let was_last = state.hub.unregister(&handle);
    if was_last {
        // Last socket gone → mark last-seen and broadcast offline.
        let now = chrono::Utc::now();
        let _ = sqlx::query("UPDATE users SET last_seen_at = $1 WHERE id = $2")
            .bind(now)
            .bind(user_id)
            .execute(&state.pool)
            .await;
        broadcast_presence(&state, user_id, false, Some(now)).await;
    }
    result
}

async fn friend_ids(state: &AppState, user_id: Uuid) -> Result<Vec<Uuid>, ()> {
    let rows = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT CASE WHEN user_id_a = $1 THEN user_id_b ELSE user_id_a END
        FROM friendships
        WHERE user_id_a = $1 OR user_id_b = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await;
    rows.map_err(|_| ())
}

/// Tells my friends I went online/offline, if my privacy setting allows.
async fn broadcast_presence(state: &AppState, user_id: Uuid, online: bool, last_seen_at: Option<chrono::DateTime<chrono::Utc>>) {
    let visibility = db::fetch_settings(&state.pool, user_id)
        .await
        .map(|s| s.online_visibility)
        .unwrap_or(Visibility::Friends);
    if visibility == Visibility::Nobody {
        return;
    }
    let Ok(friends) = friend_ids(state, user_id).await else {
        return;
    };
    state.hub.send_to_users(
        friends,
        &ServerEvent::Presence { user_id, online, last_seen_at },
    );
}

/// Used by tests to inspect online users.
pub async fn online_of(state: &AppState, user_id: Uuid) -> bool {
    state.hub.is_online(user_id)
}

#[allow(dead_code)]
fn _ensure_hashset_import_used() {
    let _s: HashSet<Uuid> = HashSet::new();
}
