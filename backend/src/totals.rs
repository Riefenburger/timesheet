use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::auth::AdminEmployee;

// --- Dollar buckets upsert (unchanged; dollars are entered, not derived) ---

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

// POST /admin/dollar-totals — create or update an employee's dollar buckets. Admin-gated.
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

// --- List totals (computed by summing entries per category + type) ---

#[derive(Deserialize)]
pub struct PeriodQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
}

#[derive(Serialize)]
pub struct CategoryRow {
    category: String,
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
}

#[derive(Serialize)]
pub struct EmployeeTotals {
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    categories: Vec<CategoryRow>,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
}

// GET /admin/totals?period_start=…&period_end=… — every employee's per-category
// hours (summed from entries, split by type) and dollar buckets. Admin-gated.
pub async fn list_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeeTotals>>, (StatusCode, String)> {
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Sum hours per employee, category (blank -> 'uncategorized'), and type.
    let sums = sqlx::query!(
        r#"
        SELECT employee_id,
               COALESCE(category, 'uncategorized') AS "category!",
               type AS "type!",
               SUM(hours) AS "hours!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2
        GROUP BY employee_id, COALESCE(category, 'uncategorized'), type
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dollars = sqlx::query!(
        r#"
        SELECT employee_id, other_earn, competition_earn, coaching_earn
        FROM period_dollar_totals
        WHERE period_start = $1 AND period_end = $2
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // employee_id -> category -> (regular, overtime, sick)
    let mut cat_map: HashMap<i64, HashMap<String, (Decimal, Decimal, Decimal)>> = HashMap::new();
    for s in &sums {
        let entry = cat_map
            .entry(s.employee_id)
            .or_default()
            .entry(s.category.clone())
            .or_insert((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));
        match s.r#type.as_str() {
            "regular" => entry.0 += s.hours,
            "overtime" => entry.1 += s.hours,
            "sick" => entry.2 += s.hours,
            _ => {}
        }
    }

    let mut dollar_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for d in &dollars {
        dollar_map.insert(d.employee_id, (d.other_earn, d.competition_earn, d.coaching_earn));
    }

    let mut result = Vec::new();
    for emp in &employees {
        let mut categories = Vec::new();
        if let Some(cats) = cat_map.get(&emp.id) {
            let mut keys: Vec<&String> = cats.keys().collect();
            keys.sort();
            for cat in keys {
                let (reg, ot, sick) = cats[cat];
                categories.push(CategoryRow {
                    category: cat.clone(),
                    regular_hours: reg,
                    overtime_hours: ot,
                    sick_hours: sick,
                });
            }
        }

        let (other, competition, coaching) = dollar_map
            .get(&emp.id).copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

        result.push(EmployeeTotals {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            categories,
            other_earn: other,
            competition_earn: competition,
            coaching_earn: coaching,
        });
    }

    Ok(Json(result))
}