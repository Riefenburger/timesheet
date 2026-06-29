use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::AdminEmployee;

#[derive(Deserialize)]
pub struct NewPeriodTotals {
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
}

#[derive(Serialize)]
pub struct PeriodTotals {
    id: i64,
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
    created_at: DateTime<Utc>,
}

// POST /admin/totals — create or update one employee's totals for a period.
pub async fn upsert_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<NewPeriodTotals>,
) -> Result<Json<PeriodTotals>, (StatusCode, String)> {
    let totals = sqlx::query_as!(
        PeriodTotals,
        r#"
        INSERT INTO period_category_totals
            (employee_id, period_start, period_end,
             regular_hours, overtime_hours, other_earn,
             competition_earn, coaching_earn, sick_hours)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (employee_id, period_start, period_end)
        DO UPDATE SET
            regular_hours    = EXCLUDED.regular_hours,
            overtime_hours   = EXCLUDED.overtime_hours,
            other_earn       = EXCLUDED.other_earn,
            competition_earn = EXCLUDED.competition_earn,
            coaching_earn    = EXCLUDED.coaching_earn,
            sick_hours       = EXCLUDED.sick_hours
        RETURNING
            id, employee_id, period_start, period_end,
            regular_hours, overtime_hours, other_earn,
            competition_earn, coaching_earn, sick_hours, created_at
        "#,
        payload.employee_id,
        payload.period_start,
        payload.period_end,
        payload.regular_hours,
        payload.overtime_hours,
        payload.other_earn,
        payload.competition_earn,
        payload.coaching_earn,
        payload.sick_hours,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(totals))
}