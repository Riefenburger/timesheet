use axum::{extract::State, http::StatusCode, Json};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::AdminEmployee;

// --- Category hours upsert (typed regular/OT/sick for a normal category) ---

#[derive(Deserialize)]
pub struct CategoryHoursInput {
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    category: String,
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
}

// POST /admin/category-hours — store one category's typed hours for a period. Admin-gated.
pub async fn upsert_category_hours(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<CategoryHoursInput>,
) -> Result<StatusCode, (StatusCode, String)> {
    sqlx::query!(
        r#"
        INSERT INTO category_hours
            (employee_id, period_start, period_end, category,
             regular_hours, overtime_hours, sick_hours)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (employee_id, period_start, period_end, category, session_duration)
        DO UPDATE SET
            regular_hours  = EXCLUDED.regular_hours,
            overtime_hours = EXCLUDED.overtime_hours,
            sick_hours     = EXCLUDED.sick_hours
        "#,
        payload.employee_id,
        payload.period_start,
        payload.period_end,
        payload.category,
        payload.regular_hours,
        payload.overtime_hours,
        payload.sick_hours,
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Private session count upsert (count for one duration in a period) ---

#[derive(Deserialize)]
pub struct PrivateCountInput {
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    session_duration: i32,
    session_count: i32,
}

// POST /admin/private-counts — store a private session count for one duration. Admin-gated.
pub async fn upsert_private_count(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<PrivateCountInput>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Deleting when count hits 0 keeps the grid clean (no zero-count private rows).
    if payload.session_count <= 0 {
        sqlx::query!(
            r#"
            DELETE FROM category_hours
            WHERE employee_id = $1 AND period_start = $2 AND period_end = $3
              AND category = 'private' AND session_duration = $4
            "#,
            payload.employee_id,
            payload.period_start,
            payload.period_end,
            payload.session_duration,
        )
        .execute(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(StatusCode::NO_CONTENT);
    }

    sqlx::query!(
        r#"
        INSERT INTO category_hours
            (employee_id, period_start, period_end, category,
             session_duration, session_count)
        VALUES ($1, $2, $3, 'private', $4, $5)
        ON CONFLICT (employee_id, period_start, period_end, category, session_duration)
        DO UPDATE SET session_count = EXCLUDED.session_count
        "#,
        payload.employee_id,
        payload.period_start,
        payload.period_end,
        payload.session_duration,
        payload.session_count,
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Dollar buckets upsert (unchanged) ---

#[derive(Deserialize)]
pub struct DollarTotalsInput {
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
}

// POST /admin/dollar-totals — store an employee's dollar buckets for a period. Admin-gated.
pub async fn upsert_dollar_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<DollarTotalsInput>,
) -> Result<StatusCode, (StatusCode, String)> {
    sqlx::query!(
        r#"
        INSERT INTO period_dollar_totals
            (employee_id, period_start, period_end,
             other_earn, competition_earn, coaching_earn)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (employee_id, period_start, period_end)
        DO UPDATE SET
            other_earn       = EXCLUDED.other_earn,
            competition_earn = EXCLUDED.competition_earn,
            coaching_earn    = EXCLUDED.coaching_earn
        "#,
        payload.employee_id,
        payload.period_start,
        payload.period_end,
        payload.other_earn,
        payload.competition_earn,
        payload.coaching_earn,
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}