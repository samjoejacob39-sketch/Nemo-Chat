//! Nook backend — Axum HTTP + WebSocket server.

mod auth;
mod config;
mod db;
mod error;
mod media;
mod routes;
mod seed;
mod state;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, DefaultBodyLimit};
use axum::Router;
use state::{AppState, Hub, RateLimiter};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nook_server=info,tower_http=info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let config = Arc::new(config::Config::from_env());

    if args.iter().any(|a| a == "--seed") {
        if let Err(e) = seed::run_seed(&config).await {
            tracing::error!("seed failed: {e}");
            std::process::exit(1);
        }
        return;
    }

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&config.database_url)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to connect to the database ({e}).");
            eprintln!("DATABASE_URL={}", config.database_url);
            eprintln!("Set up PostgreSQL and run the migrations — see README.md.");
            std::process::exit(1);
        });

    sqlx::migrate!("../../database/migrations")
        .run(&pool)
        .await
        .expect("database migrations failed");

    tokio::fs::create_dir_all(&config.media_dir)
        .await
        .expect("failed to create media directory");

    let app_state = AppState {
        pool,
        hub: Arc::new(Hub::default()),
        limiter: RateLimiter::new(),
        config: config.clone(),
    };

    // Keep the rate limiter tidy.
    {
        let limiter = app_state.limiter.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
            loop {
                interval.tick().await;
                limiter.sweep();
            }
        });
    }

    let router: Router = routes::build_router(app_state.clone())
        .layer(DefaultBodyLimit::max(config.max_upload_bytes + 1024 * 1024))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = config
        .bind
        .parse()
        .unwrap_or_else(|_| panic!("invalid NOOK_BIND '{}'", config.bind));

    tracing::info!("Nook server listening on http://{addr} (wss/ws at /api/ws)");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("server error");
}
