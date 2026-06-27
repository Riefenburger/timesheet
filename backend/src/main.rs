use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

// Incoming JSON when adding an entry. No id or created_at here —
// the database fills those in.
#[derive(Deserialize)]
struct NewEntry {
    user_name: String,
    entry_date: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    description: String,
}

// A full row as it exists in the database, sent back out as JSON.
#[derive(Serialize)]
struct TimeEntry {
    id: i64,
    user_name: String,
    entry_date: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    description: String,
    created_at: DateTime<Utc>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in backend/.env");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to the database");

    // The pool is handed to the router as shared state, so every
    // handler can reach the database via the State extractor.
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/entries", get(list_entries).post(create_entry))
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Backend listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn root_handler() -> &'static str {
    "Hello from the timesheet backend!"
}

// POST /entries — insert one row, return it (now with id and created_at).
async fn create_entry(
    State(pool): State<PgPool>,
    Json(payload): Json<NewEntry>,
) -> Result<(StatusCode, Json<TimeEntry>), (StatusCode, String)> {
    let entry = sqlx::query_as!(
        TimeEntry,
        r#"
        INSERT INTO time_entries (user_name, entry_date, hours, description)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_name, entry_date, hours, description, created_at
        "#,
        payload.user_name,
        payload.entry_date,
        payload.hours,
        payload.description
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(entry)))
}

// GET /entries — return every row, newest day first.
async fn list_entries(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<TimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        TimeEntry,
        r#"
        SELECT id, user_name, entry_date, hours, description, created_at
        FROM time_entries
        ORDER BY entry_date DESC, created_at DESC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(entries))
}