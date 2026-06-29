use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::{AdminEmployee, CurrentEmployee};

#[derive(Deserialize)]
pub struct NewTimeEntry {
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
}

#[derive(Serialize)]
pub struct TimeEntry {
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

pub async fn create_entry(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
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
        current.id,
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

pub async fn list_entries(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
) -> Result<Json<Vec<TimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        TimeEntry,
        r#"
        SELECT
            id, employee_id, entry_date, class_name, teacher_room, details, hours, created_at
        FROM time_entries
        WHERE employee_id = $1
        ORDER BY entry_date DESC, created_at DESC
        "#,
        current.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(entries))
}

// Get /admin/entries - every employee's entries. Admin-gated.
pub async fn list_all_entries(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
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