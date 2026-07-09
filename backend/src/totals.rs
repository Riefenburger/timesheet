use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::auth::AdminEmployee;

// --- Category hours upsert ---

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
    #[serde(with = "rust_decimal::serde::float")]
    logged_hours: Decimal, // sum of logged entries in this category (for seeding/mismatch)
    has_saved: bool,       // is there a saved category_hours row, or is this seed-only?
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
// hours (saved + logged) and dollar buckets for a period. Admin-gated.
pub async fn list_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeeTotals>>, (StatusCode, String)> {
    // Ingredient 1: all employees.
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 2: saved category hours for the period.
    let saved = sqlx::query!(
        r#"
        SELECT employee_id, category, regular_hours, overtime_hours, sick_hours
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 3: logged hours per employee+category for the period.
    // Blank category is reported as 'uncategorized' via COALESCE.
    let logged = sqlx::query!(
        r#"
        SELECT employee_id,
               COALESCE(category, 'uncategorized') AS "category!",
               SUM(hours) AS "hours!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2
        GROUP BY employee_id, COALESCE(category, 'uncategorized')
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 4: dollar buckets for the period.
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

    // Index the pieces by employee (and category).
    let mut saved_map: HashMap<(i64, String), (Decimal, Decimal, Decimal)> = HashMap::new();
    for s in &saved {
        saved_map.insert(
            (s.employee_id, s.category.clone()),
            (s.regular_hours, s.overtime_hours, s.sick_hours),
        );
    }

    let mut logged_map: HashMap<(i64, String), Decimal> = HashMap::new();
    for l in &logged {
        logged_map.insert((l.employee_id, l.category.clone()), l.hours);
    }

    let mut dollar_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for d in &dollars {
        dollar_map.insert(d.employee_id, (d.other_earn, d.competition_earn, d.coaching_earn));
    }

    // Assemble.
    let mut result = Vec::new();
    for emp in &employees {
        // Collect every category this employee has EITHER saved hours OR logged hours in.
        let mut cats: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for (key, _) in saved_map.iter().filter(|((eid, _), _)| *eid == emp.id) {
            cats.insert(key.1.clone());
        }
        for (key, _) in logged_map.iter().filter(|((eid, _), _)| *eid == emp.id) {
            cats.insert(key.1.clone());
        }

        let mut categories = Vec::new();
        for cat in &cats {
            let saved_vals = saved_map.get(&(emp.id, cat.clone()));
            let (reg, ot, sick) = saved_vals.copied().unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));
            let logged_hours = logged_map.get(&(emp.id, cat.clone())).copied().unwrap_or(Decimal::ZERO);
            categories.push(CategoryRow {
                category: cat.clone(),
                regular_hours: reg,
                overtime_hours: ot,
                sick_hours: sick,
                logged_hours,
                has_saved: saved_vals.is_some(),
            });
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

// POST /admin/category-hours — create or update one category's hours for an
// employee in a period. Admin-gated (regular admins manage hours).
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
        ON CONFLICT (employee_id, period_start, period_end, category)
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

// --- Dollar buckets upsert ---

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

// POST /admin/dollar-totals — create or update an employee's dollar buckets
// for a period. Admin-gated.
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