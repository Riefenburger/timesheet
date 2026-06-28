use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct NewEmployee {
    name: String,
    employee_number: String,
    email: Option<String>,
    is_admin: bool,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
}

#[derive(Serialize)]
pub struct Employee {
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

pub async fn create_employee(
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

pub async fn list_employees(
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