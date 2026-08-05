use axum::{extract::{State, Path, Query}, http::StatusCode, Json};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::{AdminEmployee, CurrentEmployee};

// For private entries, hours are derived from duration × count. For normal
// entries, the provided hours are used as-is. Returns (hours, duration, count).
fn resolve_hours(
    hours: Decimal,
    session_duration: Option<i32>,
    session_count: Option<i32>,
) -> Result<(Decimal, Option<i32>, Option<i32>), String> {
    match (session_duration, session_count) {
        (Some(dur), Some(count)) => {
            if count <= 0 {
                return Err("Session count must be positive.".to_string());
            }
            // hours = (duration / 60) * count
            let computed = Decimal::from(dur) / Decimal::from(60) * Decimal::from(count);
            Ok((computed, Some(dur), Some(count)))
        }
        (None, None) => Ok((hours, None, None)),
        _ => Err("Private sessions need both a duration and a count.".to_string()),
    }
}

// The duration CHECK is gone; validate against the managed tier list instead.
async fn validate_duration(pool: &PgPool, duration: i32) -> Result<(), (StatusCode, String)> {
    let exists = sqlx::query_scalar!(
        "SELECT 1 AS one FROM private_durations WHERE duration_minutes = $1",
        duration
    )
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::BAD_REQUEST, format!("{} minutes is not a valid private duration.", duration)));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct NewTimeEntry {
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    category: Option<String>,
    #[serde(default)]
    session_duration: Option<i32>,
    #[serde(default)]
    session_count: Option<i32>,
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
    #[serde(default)]
    session_duration: Option<i32>,
    #[serde(default)]
    session_count: Option<i32>,
    #[serde(rename = "type")]
    entry_type: String,
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
pub struct AdminEditEntry {
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    category: Option<String>,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    session_duration: Option<i32>,
    #[serde(default)]
    session_count: Option<i32>,
}

#[derive(Deserialize)]
pub struct AdminNewEntry {
    employee_id: i64,
    entry_date: NaiveDate,
    class_name: String,
    teacher_room: String,
    details: String,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    category: Option<String>,
    #[serde(rename = "type")]
    entry_type: String,
    #[serde(default)]
    session_duration: Option<i32>,
    #[serde(default)]
    session_count: Option<i32>,
}

pub async fn admin_create_entry(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<AdminNewEntry>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (hours, duration, count) = resolve_hours(
        payload.hours, payload.session_duration, payload.session_count,
    ).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if let Some(dur) = duration {
        validate_duration(&pool, dur).await?;
    }

    sqlx::query!(
        r#"
        INSERT INTO time_entries
            (employee_id, entry_date, class_name, teacher_room, details, hours,
             category, type, session_duration, session_count)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
        payload.employee_id,
        payload.entry_date,
        payload.class_name,
        payload.teacher_room,
        payload.details,
        hours,
        payload.category,
        payload.entry_type,
        duration,
        count,
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// DELETE /admin/entries/:id — remove an entry. Admin-gated.
pub async fn admin_delete_entry(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Path(entry_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        "DELETE FROM time_entries WHERE id = $1",
        entry_id
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Entry not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// PUT /admin/entries/:id — edit any field of an entry. Admin-gated.
pub async fn admin_edit_entry(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Path(entry_id): Path<i64>,
    Json(payload): Json<AdminEditEntry>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (hours, duration, count) = resolve_hours(
        payload.hours, payload.session_duration, payload.session_count,
    ).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if let Some(dur) = duration {
        validate_duration(&pool, dur).await?;
    }

    let result = sqlx::query!(
        r#"
        UPDATE time_entries
        SET entry_date = $1, class_name = $2, teacher_room = $3,
            details = $4, hours = $5, category = $6, type = $7,
            session_duration = $8, session_count = $9
        WHERE id = $10
        "#,
        payload.entry_date,
        payload.class_name,
        payload.teacher_room,
        payload.details,
        hours,
        payload.category,
        payload.entry_type,
        duration,
        count,
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
    let (hours, duration, count) = resolve_hours(
        payload.hours, payload.session_duration, payload.session_count,
    ).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if let Some(dur) = duration {
        validate_duration(&pool, dur).await?;
    }

    let entry = sqlx::query_as!(
        TimeEntry,
        r#"
        INSERT INTO time_entries
            (employee_id, entry_date, class_name, teacher_room, details, hours,
             category, type, session_duration, session_count)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'regular', $8, $9)
        RETURNING
            id, employee_id, entry_date, class_name, teacher_room, details, hours,
            category, type AS "entry_type!", session_duration, session_count, created_at
        "#,
        current.id,
        payload.entry_date,
        payload.class_name,
        payload.teacher_room,
        payload.details,
        hours,
        payload.category,
        duration,
        count,
    )
    .fetch_one(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(entry)))
}

#[derive(Deserialize)]
pub struct MyEntriesQuery {
    period_start: Option<NaiveDate>,
    period_end: Option<NaiveDate>,
}

pub async fn list_entries(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
    Query(q): Query<MyEntriesQuery>,
) -> Result<Json<Vec<TimeEntry>>, (StatusCode, String)> {
    let entries = sqlx::query_as!(
        TimeEntry,
        r#"
        SELECT
            id, employee_id, entry_date, class_name, teacher_room, details, hours, category,
            session_duration, session_count,
            type AS "entry_type!",
            created_at
        FROM time_entries
        WHERE employee_id = $1
          AND ($2::date IS NULL OR entry_date >= $2)
          AND ($3::date IS NULL OR entry_date <= $3)
        ORDER BY entry_date DESC, created_at DESC
        "#,
        current.id,
        q.period_start,
        q.period_end,
    )
    .fetch_all(&pool).await
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
            id, employee_id, entry_date, class_name, teacher_room, details, hours, category,
            session_duration, session_count,
            type AS "entry_type!",
            created_at
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

#[derive(Serialize)]
pub struct MyCategories {
    // Normal category names the employee has rates for (e.g. ["office","teaching"]).
    categories: Vec<String>,
    // Private durations the employee has rates for (e.g. [20, 30]). Empty if none.
    private_durations: Vec<i32>,
}

// GET /entries/categories — which categories (and private durations) the CURRENT
// employee has rates for. Drives the entry form's category + duration pickers.
pub async fn my_categories(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
) -> Result<Json<MyCategories>, (StatusCode, String)> {
        // Valid duration tiers (for filtering the employee's private_NN rates).
    let valid_durations: Vec<i32> = sqlx::query_scalar!(
        r#"SELECT duration_minutes FROM private_durations"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // The employee's rate labels (normal categories + private_NN).
    let labels: Vec<String> = sqlx::query_scalar!(
        r#"SELECT DISTINCT label FROM employee_rates WHERE employee_id = $1 ORDER BY label"#,
        current.id
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Split private_NN labels out into durations; everything else is a normal category.
    let mut categories = Vec::new();
    let mut private_durations = Vec::new();
    for label in labels {
        if let Some(rest) = label.strip_prefix("private_") {
            if let Ok(dur) = rest.parse::<i32>() {
                if valid_durations.contains(&dur) {
                    private_durations.push(dur);
                }
            }
        } else {
            categories.push(label);
        }
    }
    private_durations.sort();

    Ok(Json(MyCategories { categories, private_durations }))
}