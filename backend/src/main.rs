use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    // Load variables from .env into the process environment.
    dotenvy::dotenv().ok();

    // Read the connection string from backend/.env
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in backend/.env");

    // A connection pool: a managed set of reusable Postgres connections,
    // so we aren't opening a brand-new one on every request.
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to the database");

    // Smoke test: have Postgres compute 1 + 1. If this returns a value,
    // the pool is genuinely talking to the database
    let answer: i32 = sqlx::query_scalar("SELECT 1 + 1")
        .fetch_one(&pool)
        .await
        .expect("Test query failed");
    println!("Database connected - test query returned {answer}");

    // A Router maps URL paths to handler functions.
    // This says: a GET request to "/" runs root_handler.
    let app = Router::new().route("/", get(root_handler));

    // Bind a TCP port, then hand the router to axum::serve
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Backend listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

// A handler is an async function returing something axum can turn into
// a response. A string becomes a 200 OK with that text as the body.
async fn root_handler() -> &'static str {
    "Hello form the timesheet backend!"
}