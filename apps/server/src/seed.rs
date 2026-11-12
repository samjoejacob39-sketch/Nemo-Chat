//! Development seed data: two friendly demo accounts who are already
//! friends with a short conversation. Run with `nook-server --seed`.
//!
//! Only for local development — refuses to run against remote-looking
//! databases.

use std::sync::Arc;

use uuid::Uuid;

use crate::auth;
use crate::config::Config;

pub struct SeedUser {
    pub username: &'static str,
    pub password: &'static str,
    pub display_name: &'static str,
    pub bio: &'static str,
}

const USERS: [SeedUser; 2] = [
    SeedUser {
        username: "alice",
        password: "applepie2026",
        display_name: "Alice Chen",
        bio: "Coffee, code and long walks. ☕",
    },
    SeedUser {
        username: "bob",
        password: "bananabread1",
        display_name: "Bob Rivera",
        bio: "Playing bass and building keyboards.",
    },
];

pub async fn run_seed(config: &Arc<Config>) -> Result<(), Box<dyn std::error::Error>> {
    let host = config
        .database_url
        .split('@')
        .nth(1)
        .unwrap_or_default()
        .split('/')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string();
    if !["localhost", "127.0.0.1", "::1"].contains(&host.as_str()) {
        return Err(format!("refusing to seed a remote database ({host})").into());
    }

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("../../database/migrations").run(&pool).await?;

    let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE username = ANY($1)")
        .bind(&["alice".to_string(), "bob".to_string()])
        .fetch_one(&pool)
        .await?;
    if existing > 0 {
        println!("Seed users already exist — nothing to do.");
        return Ok(());
    }

    let mut ids = Vec::new();
    for u in &USERS {
        let id = Uuid::now_v7();
        let hash = auth::hash_password(u.password)?;
        sqlx::query("INSERT INTO users (id, username, display_name, bio, password_hash) VALUES ($1,$2,$3,$4,$5)")
            .bind(id)
            .bind(u.username)
            .bind(u.display_name)
            .bind(u.bio)
            .bind(&hash)
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO user_settings (user_id) VALUES ($1)")
            .bind(id)
            .execute(&pool)
            .await?;
        // Friendly recovery codes so the demo accounts can be recovered too.
        let code = format!("NOOK-{}-{}-{}", u.username.to_uppercase(), "DEMO", "2026");
        let hash = auth::hash_password(&auth::normalize_recovery_code(&code))?;
        sqlx::query("INSERT INTO recovery_codes (id, user_id, code_hash) VALUES ($1,$2,$3)")
            .bind(Uuid::now_v7())
            .bind(id)
            .bind(&hash)
            .execute(&pool)
            .await?;
        println!("seeded @{:<8} password: {:<14} recovery: {}", u.username, u.password, code);
        ids.push(id);
    }

    let (a, b) = (ids[0], ids[1]);
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };

    sqlx::query("INSERT INTO friendships (id, user_id_a, user_id_b) VALUES ($1,$2,$3)")
        .bind(Uuid::now_v7())
        .bind(lo)
        .bind(hi)
        .execute(&pool)
        .await?;

    let dm_key = format!("{lo}|{hi}");
    let conv: Uuid = sqlx::query_scalar(
        "INSERT INTO conversations (id, kind, dm_key) VALUES ($1,'private',$2) RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&dm_key)
    .fetch_one(&pool)
    .await?;
    for uid in [a, b] {
        sqlx::query("INSERT INTO conversation_members (id, conversation_id, user_id, role) VALUES ($1,$2,$3,'member')")
            .bind(Uuid::now_v7())
            .bind(conv)
            .bind(uid)
            .execute(&pool)
            .await?;
    }

    let sample = [
        (a, "Hey Bob! Nook is live 🎉"),
        (b, "It looks so clean. Love the glass nav bar."),
        (a, "Send me a photo when you get a chance?"),
    ];
    for (sender, body) in sample {
        sqlx::query("INSERT INTO messages (id, conversation_id, sender_id, kind, body) VALUES ($1,$2,$3,'text',$4)")
            .bind(Uuid::now_v7())
            .bind(conv)
            .bind(sender)
            .bind(body)
            .execute(&pool)
            .await?;
    }
    sqlx::query("UPDATE conversations SET last_message_at = now() WHERE id = $1")
        .bind(conv)
        .execute(&pool)
        .await?;

    println!("Seed complete: alice & bob are friends with a short conversation.");
    Ok(())
}
