mod auth;
mod employees;
mod entries;
mod totals;
mod rates;
mod pay;

use axum::{routing::{get, post, put, delete}, Router};
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
        .route("/employees", get(employees::list_employees).post(employees::create_employee))
        .route("/entries", get(entries::list_entries).post(entries::create_entry))
        .route("/admin/entries", get(entries::list_all_entries))
        .route("/admin/totals", get(totals::list_period_totals).post(totals::upsert_totals))
        .route("/admin/employees/{id}/entries", get(entries::list_employee_entries))
        .route("/admin/totals/export", get(totals::export_totals))
        .route("/admin/employees/{id}/rates", get(rates::list_rates))
        .route("/admin/rates", post(rates::create_rate))
        .route("/admin/rates/{id}", put(rates::update_rate).delete(rates::delete_rate))
        .route("/entries/categories", get(entries::my_categories))
        .route("/admin/pay", get(pay::list_pay))
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