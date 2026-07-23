use axum::{
    routing::post,
    Router,
    Json,
    extract::State,
    http::Method,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool, Row};
use std::str::FromStr;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[derive(Deserialize, Debug)]
struct TelemetryPayload {
    os_name: String,
    cpu_name: String,
    core_count: i64,
    memory_total_mb: i64,
    cpu_score: i64,
    gpu_score: i64,
    ram_score: i64,
    disk_score: i64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nemdiag_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup SQLite Connection with WAL mode
    let options = SqliteConnectOptions::from_str("sqlite://data/nemdiag.db")
        .unwrap()
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal); // WAL for concurrency

    // Ensure data directory exists
    tokio::fs::create_dir_all("data").await.unwrap();

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to connect to SQLite");

    // Initialize Schema
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS telemetry (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            os_name TEXT,
            cpu_name TEXT,
            core_count INTEGER,
            memory_total_mb INTEGER,
            cpu_score INTEGER,
            gpu_score INTEGER,
            ram_score INTEGER,
            disk_score INTEGER
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create telemetry table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS crashes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
            log TEXT
        );"
    )
    .execute(&pool)
    .await
    .expect("Failed to create crashes table");

    let state = AppState { db: pool };

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/telemetry", post(handle_telemetry))
        .route("/api/crash", post(handle_crash))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("Server listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handle_telemetry(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TelemetryPayload>,
) -> String {
    let result = sqlx::query(
        "INSERT INTO telemetry (os_name, cpu_name, core_count, memory_total_mb, cpu_score, gpu_score, ram_score, disk_score) 
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(payload.os_name)
    .bind(payload.cpu_name)
    .bind(payload.core_count)
    .bind(payload.memory_total_mb)
    .bind(payload.cpu_score)
    .bind(payload.gpu_score)
    .bind(payload.ram_score)
    .bind(payload.disk_score)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Saved telemetry data to SQLite.");
            "OK".to_string()
        },
        Err(e) => {
            tracing::error!("Failed to insert telemetry: {}", e);
            "Error".to_string()
        }
    }
}

async fn handle_crash(
    State(state): State<Arc<AppState>>,
    body: String,
) -> String {
    let result = sqlx::query("INSERT INTO crashes (log) VALUES (?)")
        .bind(body)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Saved crash log to SQLite.");
            "OK".to_string()
        },
        Err(e) => {
            tracing::error!("Failed to insert crash log: {}", e);
            "Error".to_string()
        }
    }
}
