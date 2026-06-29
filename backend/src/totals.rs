use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
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

// The period comes in through the URL: ?period_start=…&period_end=…
#[derive(Deserialize)]
pub struct PeriodQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
}

// One row per employee for the period. Totals are Option — null when the
// admin hasn't entered anything for that employee in that period yet.
#[derive(Serialize)]
pub struct EmployeePeriodTotals {
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    totals_id: Option<i64>,
    #[serde(with = "rust_decimal::serde::float_option")]
    regular_hours: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    overtime_hours: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    other_earn: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    competition_earn: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    coaching_earn: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::float_option")]
    sick_hours: Option<Decimal>,
}

// GET /admin/totals?period_start=…&period_end=… — every employee, totals or blanks.
pub async fn list_period_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeePeriodTotals>>, (StatusCode, String)> {
    let rows = sqlx::query_as!(
        EmployeePeriodTotals,
        r#"
        SELECT
            e.id               AS "employee_id!",
            e.name             AS "employee_name!",
            e.employee_number  AS "employee_number!",
            t.id               AS "totals_id?",
            t.regular_hours    AS "regular_hours?",
            t.overtime_hours   AS "overtime_hours?",
            t.other_earn       AS "other_earn?",
            t.competition_earn AS "competition_earn?",
            t.coaching_earn    AS "coaching_earn?",
            t.sick_hours       AS "sick_hours?"
        FROM employees e
        LEFT JOIN period_category_totals t
            ON t.employee_id = e.id
            AND t.period_start = $1
            AND t.period_end = $2
        ORDER BY e.name
        "#,
        period.period_start,
        period.period_end,
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
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