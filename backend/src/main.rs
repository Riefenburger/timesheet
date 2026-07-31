mod auth;
mod employees;
mod entries;
mod totals;
mod rates;
mod categories;

use axum::{routing::{get, post, put}, Router};
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
        .route("/employees", get(employees::list_employees))
        .route("/admin/employees", post(employees::create_employee))
        .route("/entries", get(entries::list_entries).post(entries::create_entry))
        .route("/admin/entries", get(entries::list_all_entries).post(entries::admin_create_entry))
        .route("/admin/entries/{id}", axum::routing::put(entries::admin_edit_entry).delete(entries::admin_delete_entry))
        .route("/admin/employees/{id}", put(employees::update_employee))
        .route("/admin/employees/{id}/entries", get(entries::list_employee_entries))
        .route("/admin/employees/{id}/rates", get(rates::list_rates))
        .route("/admin/rates", post(rates::create_rate))
        .route("/admin/rates/{id}", put(rates::update_rate).delete(rates::delete_rate))
        .route("/entries/categories", get(entries::my_categories))
        .route("/admin/totals", get(totals::list_totals))
        .route("/categories", get(categories::list_categories))
        .route("/admin/categories", post(categories::create_category))
        .route("/admin/categories/{id}", put(categories::update_category).delete(categories::delete_category))
        .route("/admin/category-hours", post(totals::upsert_category_hours))
        .route("/admin/private-counts", post(totals::upsert_private_count))
        .route("/admin/dollar-totals", post(totals::upsert_dollar_totals))
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