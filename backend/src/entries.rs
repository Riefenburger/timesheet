use axum::{extract::{State, Path, Query}, http::StatusCode, Json};
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
    category: Option<String>,
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
    category: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AdminTimeEntry {
    id: i64,
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    category: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct EntryPeriodQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
    category: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateEntryClassification {
    category: Option<String>,
    #[serde(rename = "type")]
    entry_type: String,
}

// PATCH /admin/entries/:id — change an entry's category and/or type. Admin-gated.
pub async fn update_entry_classification(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Path(entry_id): Path<i64>,
    Json(payload): Json<UpdateEntryClassification>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"
        UPDATE time_entries
        SET category = $1, type = $2
        WHERE id = $3
        "#,
        payload.category,
        payload.entry_type,
        entry_id,
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Entry not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
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
            (employee_id, entry_date, class_name, teacher_room, details, hours, category)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, employee_id, entry_date, class_name, teacher_room, details, hours, category, created_at
        "#,
        current.id,
        payload.entry_date,
        payload.class_name,
        payload.teacher_room,
        payload.details,
        payload.hours,
        payload.category,
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
            id, employee_id, entry_date, class_name, teacher_room, details, hours, category, created_at
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

// GET /admin/entries — every employee's entries, WITH their name. Admin-gated.
pub async fn list_all_entries(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
) -> Result<Json<Vec<AdminTimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        AdminTimeEntry,
        r#"
        SELECT
            t.id,
            t.employee_id,
            e.name           AS employee_name,
            e.employee_number AS employee_number,
            t.entry_date,
            t.class_name,
            t.teacher_room,
            t.details,
            t.hours,
            t.category,
            t.created_at
        FROM time_entries t
        JOIN employees e ON e.id = t.employee_id
        ORDER BY e.name, t.entry_date DESC
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(entries))
}

// GET /admin/employees/:id/entries?period_start=…&period_end=…
// One employee's sessions within a period. Admin-gated.
pub async fn list_employee_entries(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Path(employee_id): Path<i64>,
    Query(period): Query<EntryPeriodQuery>,
) -> Result<Json<Vec<TimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        TimeEntry,
        r#"
        SELECT
            id, employee_id, entry_date, class_name, teacher_room, details, hours, category, created_at
        FROM time_entries
        WHERE employee_id = $1
          AND entry_date BETWEEN $2 AND $3
          AND (
            $4::text IS NULL
            OR ($4 = '__uncategorized__' AND category IS NULL)
            OR category = $4
          )
        ORDER BY entry_date
        "#,
        employee_id,
        period.period_start,
        period.period_end,
        period.category,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(entries))
}

// GET /entries/categories — which categories the CURRENT employee has rates for.
// Drives the entry form's category dropdown.
pub async fn my_categories(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let labels = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT label
        FROM employee_rates
        WHERE employee_id = $1
        ORDER BY label
        "#,
        current.id
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(labels))
}