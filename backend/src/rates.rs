use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::SuperAdminEmployee;

#[derive(Serialize)]
pub struct Rate {
    id: i64,
    employee_id: i64,
    label: String,
    #[serde(with = "rust_decimal::serde::float")]
    amount: Decimal,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct NewRate {
    employee_id: i64,
    label: String,
    #[serde(with = "rust_decimal::serde::float")]
    amount: Decimal,
}

#[derive(Deserialize)]
pub struct UpdateRate {
    label: String,
    #[serde(with = "rust_decimal::serde::float")]
    amount: Decimal,
}

// GET /admin/employees/:id/rates — one employee's rates. Super-admin only.
pub async fn list_rates(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
) -> Result<Json<Vec<Rate>>, (StatusCode, String)> {
    let rates = sqlx::query_as!(
        Rate,
        r#"
        SELECT id, employee_id, label, amount, created_at
        FROM employee_rates
        WHERE employee_id = $1
        ORDER BY label
        "#,
        employee_id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rates))
}

// POST /admin/rates — add a rate. Super-admin only.
pub async fn create_rate(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<NewRate>,
) -> Result<(StatusCode, Json<Rate>), (StatusCode, String)> {
    let rate = sqlx::query_as!(
        Rate,
        r#"
        INSERT INTO employee_rates (employee_id, label, amount)
        VALUES ($1, $2, $3)
        RETURNING id, employee_id, label, amount, created_at
        "#,
        payload.employee_id,
        payload.label,
        payload.amount,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(rate)))
}

// PUT /admin/rates/:id — edit a rate's label or amount. Super-admin only.
pub async fn update_rate(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(rate_id): Path<i64>,
    Json(payload): Json<UpdateRate>,
) -> Result<Json<Rate>, (StatusCode, String)> {
    let rate = sqlx::query_as!(
        Rate,
        r#"
        UPDATE employee_rates
        SET label = $1, amount = $2
        WHERE id = $3
        RETURNING id, employee_id, label, amount, created_at
        "#,
        payload.label,
        payload.amount,
        rate_id,
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Rate not found".to_string()))?;

    Ok(Json(rate))
}

// DELETE /admin/rates/:id — remove a rate. Super-admin only.
pub async fn delete_rate(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(rate_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        "DELETE FROM employee_rates WHERE id = $1",
        rate_id
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Rate not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}