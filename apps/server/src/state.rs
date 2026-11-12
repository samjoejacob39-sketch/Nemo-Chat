//! Shared application state: database pool, real-time hub, rate limiter,
//! media storage and configuration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use nook_shared::ServerEvent;
use sqlx::PgPool;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::Config;

/// A single live WebSocket connection.
pub type ConnSink = mpsc::UnboundedSender<ServerEvent>;
type ConnMap = HashMap<Uuid /* conn id */, ConnSink>;

/// Routes events to the live sockets of every online user.
#[derive(Default)]
pub struct Hub {
    conns: DashMap<Uuid, ConnMap>,
    conn_users: DashMap<Uuid /* conn id */, Uuid /* user id */>,
}

pub struct ConnHandle {
    pub user_id: Uuid,
    pub conn_id: Uuid,
}

impl Hub {
    pub fn register(&self, user_id: Uuid) -> (ConnHandle, mpsc::UnboundedReceiver<ServerEvent>) {
        let conn_id = Uuid::now_v7();
        let (tx, rx) = mpsc::unbounded_channel();
        self.conn_users.insert(conn_id, user_id);
        self.conns.entry(user_id).or_default().insert(conn_id, tx);
        (ConnHandle { user_id, conn_id }, rx)
    }

    /// Removes a connection; returns true if this was the user's last one.
    pub fn unregister(&self, handle: &ConnHandle) -> bool {
        self.conn_users.remove(&handle.conn_id);
        let was_last = if let Some(mut map) = self.conns.get_mut(&handle.user_id) {
            map.value_mut().remove(&handle.conn_id);
            map.is_empty()
        } else {
            false
        };
        if was_last {
            self.conns.remove(&handle.user_id);
        }
        was_last
    }

    pub fn is_online(&self, user_id: Uuid) -> bool {
        self.conns.contains_key(&user_id)
    }

    pub fn online_subset(&self, users: &[Uuid]) -> Vec<Uuid> {
        users.iter().copied().filter(|u| self.is_online(*u)).collect()
    }

    /// Sends an event to every live socket of one user.
    pub fn send_to_user(&self, user_id: Uuid, event: &ServerEvent) {
        if let Some(map) = self.conns.get(&user_id) {
            for (_id, tx) in map.value().iter() {
                let _ = tx.send(event.clone());
            }
        }
    }

    pub fn send_to_users<I: IntoIterator<Item = Uuid>>(&self, users: I, event: &ServerEvent) {
        for user in users {
            self.send_to_user(user, event);
        }
    }
}

// ---------------------------------------------------------------------------
// Rate limiting: tiny token-bucket limiter keyed by (subject, class).
// ---------------------------------------------------------------------------

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    buckets: Arc<DashMap<String, Bucket>>,
}

pub struct RateClass {
    pub name: &'static str,
    pub capacity: f64,
    pub refill_per_sec: f64,
}

impl RateClass {
    const fn new(name: &'static str, per_minute: f64) -> Self {
        Self { name, capacity: per_minute, refill_per_sec: per_minute / 60.0 }
    }
}

pub static RATE_AUTH: RateClass = RateClass::new("auth", 10.0);
pub static RATE_SEARCH: RateClass = RateClass::new("search", 30.0);
pub static RATE_FRIEND_REQUEST: RateClass = RateClass::new("friend_request", 10.0);
pub static RATE_MESSAGE: RateClass = RateClass::new("message", 60.0);
pub static RATE_REACTION: RateClass = RateClass::new("reaction", 60.0);
pub static RATE_UPLOAD: RateClass = RateClass::new("upload", 20.0);
pub static RATE_REPORT: RateClass = RateClass::new("report", 10.0);
pub static RATE_WS: RateClass = RateClass::new("ws", 240.0);

impl RateLimiter {
    pub fn new() -> Self {
        Self { buckets: Arc::new(DashMap::new()) }
    }

    /// Returns an error message when the caller should back off.
    pub fn check(&self, subject: &str, class: &RateClass) -> Result<(), String> {
        let key = format!("{subject}:{}", class.name);
        let now = Instant::now();
        let mut entry = self.buckets.entry(key).or_insert(Bucket {
            tokens: class.capacity,
            last_refill: now,
        });
        let bucket = entry.value_mut();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * class.refill_per_sec).min(class.capacity);
        bucket.last_refill = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else {
            Err("You're doing that a bit too quickly. Please slow down a moment.".into())
        }
    }

    /// Periodically clears idle buckets.
    pub fn sweep(&self) {
        let cutoff = Instant::now() - Duration::from_secs(300);
        self.buckets.retain(|_, b| b.last_refill > cutoff);
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub hub: Arc<Hub>,
    pub limiter: RateLimiter,
    pub config: Arc<Config>,
}
