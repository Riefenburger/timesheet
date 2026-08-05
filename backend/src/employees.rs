use axum::{extract::{State, Path}, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::auth::SuperAdminEmployee;

#[derive(Deserialize)]
pub struct NewEmployee {
    name: String,
    employee_number: String,
    email: Option<String>,
    role: String,
    pay_method: String,
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
    role: String,
    pay_method: String,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
    has_account: bool,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdateEmployee {
    name: String,
    employee_number: String,
    email: Option<String>,
    role: String,
    pay_method: String,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
}

pub async fn create_employee(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<NewEmployee>,
) -> Result<(StatusCode, Json<Employee>), (StatusCode, String)> {
    // Friendly check: is the employee number already taken?
    let conflict = sqlx::query!(
        "SELECT name FROM employees WHERE employee_number = $1",
        payload.employee_number
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(row) = conflict {
        return Err((
            StatusCode::CONFLICT,
            format!("Employee number {} is already used by {}.", payload.employee_number, row.name),
        ));
    }

    let employee = sqlx::query_as!(
        Employee,
        r#"
        INSERT INTO employees
            (name, employee_number, email, role, pay_method, is_salaried, salary)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, name, employee_number, email, google_sub,
            role, pay_method, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            created_at
        "#,
        payload.name,
        payload.employee_number,
        payload.email,
        payload.role,
        payload.pay_method,
        payload.is_salaried,
        payload.salary,
    )
    .fetch_one(&pool).await
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
            role, pay_method, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            created_at
        FROM employees
        ORDER BY name
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employees))
}

// PUT /admin/employees/:id — update an employee. Super-admin only.
pub async fn update_employee(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
    Json(payload): Json<UpdateEmployee>,
) -> Result<Json<Employee>, (StatusCode, String)> {
    // --- Guard: don't let the last super_admin lose that role ---
    // If this employee is currently super_admin and the new role isn't,
    // make sure at least one OTHER super_admin remains.
    let current_role = sqlx::query_scalar!(
        "SELECT role FROM employees WHERE id = $1",
        employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Employee not found".to_string()))?;

    if current_role == "super_admin" && payload.role != "super_admin" {
        let other_supers = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM employees WHERE role = 'super_admin' AND id != $1"#,
            employee_id
        )
        .fetch_one(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if other_supers == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "Cannot remove the last super-admin. Promote someone else first.".to_string(),
            ));
        }
    }

    // --- Friendly check: is the employee number taken by someone else? ---
    let conflict = sqlx::query!(
        "SELECT name FROM employees WHERE employee_number = $1 AND id != $2",
        payload.employee_number,
        employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(row) = conflict {
        return Err((
            StatusCode::CONFLICT,
            format!("Employee number {} is already used by {}.", payload.employee_number, row.name),
        ));
    }

    // --- Do the update ---
    let employee = sqlx::query_as!(
        Employee,
        r#"
        UPDATE employees
        SET name = $1, employee_number = $2, email = $3,
            role = $4, pay_method = $5, is_salaried = $6, salary = $7
        WHERE id = $8
        RETURNING
            id, name, employee_number, email, google_sub,
            role, pay_method, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            created_at
        "#,
        payload.name,
        payload.employee_number,
        payload.email,
        payload.role,
        payload.pay_method,
        payload.is_salaried,
        payload.salary,
        employee_id,
    )
    .fetch_one(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employee))
}