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

// ===== Employees =====

#[derive(Deserialize)]
struct NewEmployee {
    name: String,
    employee_number: String,
    email: Option<String>,
    is_admin: bool,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
}

#[derive(Serialize)]
struct Employee {
    id: i64,
    name: String,
    employee_number: String,
    email: Option<String>,
    google_sub: Option<String>,
    is_admin: bool,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
    created_at: DateTime<Utc>,
}

// ===== Time entries =====

#[derive(Deserialize)]
struct NewTimeEntry {
    employee_id: i64,
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
}

#[derive(Serialize)]
struct TimeEntry {
    id: i64,
    employee_id: i64,
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
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

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/employees", get(list_employees).post(create_employee))
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

// ----- Employee handlers -----

async fn create_employee(
    State(pool): State<PgPool>,
    Json(payload): Json<NewEmployee>,
) -> Result<(StatusCode, Json<Employee>), (StatusCode, String)> {
    let employee = sqlx::query_as!(
        Employee,
        r#"
        INSERT INTO employees
            (name, employee_number, email, is_admin, is_salaried, salary)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING
            id, name, employee_number, email, google_sub,
            is_admin, is_salaried, salary, created_at
        "#,
        payload.name,
        payload.employee_number,
        payload.email,
        payload.is_admin,
        payload.is_salaried,
        payload.salary,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(employee)))
}

async fn list_employees(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Employee>>, (StatusCode, String)> {
    let employees = sqlx::query_as!(
        Employee,
        r#"
        SELECT
            id, name, employee_number, email, google_sub,
            is_admin, is_salaried, salary, created_at
        FROM employees
        ORDER BY name
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employees))
}

// ----- Time entry handlers -----

async fn create_entry(
    State(pool): State<PgPool>,
    Json(payload): Json<NewTimeEntry>,
) -> Result<(StatusCode, Json<TimeEntry>), (StatusCode, String)> {
    let entry = sqlx::query_as!(
        TimeEntry,
        r#"
        INSERT INTO time_entries
            (employee_id, entry_date, class_name, teacher_room, details, hours)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING
            id, employee_id, entry_date, class_name, teacher_room, details, hours, created_at
        "#,
        payload.employee_id,
        payload.entry_date,
        payload.class_name,
        payload.teacher_room,
        payload.details,
        payload.hours,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(entry)))
}

async fn list_entries(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<TimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        TimeEntry,
        r#"
        SELECT
            id, employee_id, entry_date, class_name, teacher_room, details, hours, created_at
        FROM time_entries
        ORDER BY entry_date DESC, created_at DESC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(entries))
}