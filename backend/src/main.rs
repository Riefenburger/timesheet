mod auth;
mod employees;
mod entries;

use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;

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

    let app = Router::new()
        .route("/", get(root_handler))
        .route(
            "/employees",
            get(employees::list_employees).post(employees::create_employee),
        )
        .route(
            "/entries",
            get(entries::list_entries).post(entries::create_entry),
        )
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