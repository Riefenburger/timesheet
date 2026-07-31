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
    // Sum of logged entry hours in this category (for the mismatch warning).
    #[serde(with = "rust_decimal::serde::float")]
    logged_hours: Decimal,
}

#[derive(Serialize)]
pub struct PrivateRow {
    session_duration: i32,
    session_count: i32,        // stored/typed count
    logged_count: i32,         // summed from logged private entries (mismatch)
}

#[derive(Serialize)]
pub struct EmployeeTotals {
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    pay_method: String,
    categories: Vec<CategoryRow>,
    private_sessions: Vec<PrivateRow>,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
}

// GET /admin/totals?period_start=…&period_end=… — every employee's stored hours
// and private counts, with logged sums for the mismatch, plus dollars. Admin-gated.
pub async fn list_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeeTotals>>, (StatusCode, String)> {
    // 1. All employees.
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number, pay_method FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Stored normal category hours (non-private rows).
    let stored_cats = sqlx::query!(
        r#"
        SELECT employee_id, category, regular_hours, overtime_hours, sick_hours
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2 AND session_duration IS NULL
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. Stored private session counts (private rows).
    let stored_privates = sqlx::query!(
        r#"
        SELECT employee_id, session_duration AS "session_duration!", session_count AS "session_count!"
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2 AND session_duration IS NOT NULL
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 4. Logged normal hours per category (non-private entries).
    let logged_cats = sqlx::query!(
        r#"
        SELECT employee_id,
               COALESCE(category, 'uncategorized') AS "category!",
               SUM(hours) AS "hours!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2 AND session_duration IS NULL
        GROUP BY employee_id, COALESCE(category, 'uncategorized')
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 5. Logged private counts per duration (private entries).
    let logged_privates = sqlx::query!(
        r#"
        SELECT employee_id,
               session_duration AS "session_duration!",
               SUM(session_count)::int AS "count!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2 AND session_duration IS NOT NULL
        GROUP BY employee_id, session_duration
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 6. Dollar buckets.
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

    // --- Index everything by employee ---

    // Stored category hours: employee -> category -> (reg, ot, sick)
    let mut stored_map: HashMap<i64, HashMap<String, (Decimal, Decimal, Decimal)>> = HashMap::new();
    for s in &stored_cats {
        stored_map.entry(s.employee_id).or_default()
            .insert(s.category.clone(), (s.regular_hours, s.overtime_hours, s.sick_hours));
    }

    // Logged category hours: employee -> category -> hours
    let mut logged_map: HashMap<i64, HashMap<String, Decimal>> = HashMap::new();
    for l in &logged_cats {
        logged_map.entry(l.employee_id).or_default().insert(l.category.clone(), l.hours);
    }

    // Stored private: employee -> duration -> count
    let mut stored_priv_map: HashMap<i64, HashMap<i32, i32>> = HashMap::new();
    for p in &stored_privates {
        stored_priv_map.entry(p.employee_id).or_default().insert(p.session_duration, p.session_count);
    }

    // Logged private: employee -> duration -> count
    let mut logged_priv_map: HashMap<i64, HashMap<i32, i32>> = HashMap::new();
    for p in &logged_privates {
        logged_priv_map.entry(p.employee_id).or_default().insert(p.session_duration, p.count);
    }

    let mut dollar_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for d in &dollars {
        dollar_map.insert(d.employee_id, (d.other_earn, d.competition_earn, d.coaching_earn));
    }

    // --- Assemble ---
    let mut result = Vec::new();
    for emp in &employees {
        // Categories: union of stored and logged category names.
        let mut cat_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if let Some(m) = stored_map.get(&emp.id) { for k in m.keys() { cat_names.insert(k.clone()); } }
        if let Some(m) = logged_map.get(&emp.id) { for k in m.keys() { cat_names.insert(k.clone()); } }

        let mut categories = Vec::new();
        for cat in &cat_names {
            let (reg, ot, sick) = stored_map.get(&emp.id)
                .and_then(|m| m.get(cat)).copied()
                .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));
            let logged = logged_map.get(&emp.id)
                .and_then(|m| m.get(cat)).copied()
                .unwrap_or(Decimal::ZERO);
            categories.push(CategoryRow {
                category: cat.clone(),
                regular_hours: reg,
                overtime_hours: ot,
                sick_hours: sick,
                logged_hours: logged,
            });
        }

        // Private: union of stored and logged durations.
        let mut durations: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
        if let Some(m) = stored_priv_map.get(&emp.id) { for k in m.keys() { durations.insert(*k); } }
        if let Some(m) = logged_priv_map.get(&emp.id) { for k in m.keys() { durations.insert(*k); } }

        let mut private_sessions = Vec::new();
        for dur in &durations {
            let count = stored_priv_map.get(&emp.id).and_then(|m| m.get(dur)).copied().unwrap_or(0);
            let logged = logged_priv_map.get(&emp.id).and_then(|m| m.get(dur)).copied().unwrap_or(0);
            private_sessions.push(PrivateRow {
                session_duration: *dur,
                session_count: count,
                logged_count: logged,
            });
        }

        let (other, competition, coaching) = dollar_map.get(&emp.id).copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

        result.push(EmployeeTotals {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            pay_method: emp.pay_method.clone(),
            categories,
            private_sessions,
            other_earn: other,
            competition_earn: competition,
            coaching_earn: coaching,
        });
    }

    Ok(Json(result))
}