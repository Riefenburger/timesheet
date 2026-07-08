use std::collections::HashMap;

use axum::{extract::{Query, State}, http::StatusCode, Json};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::SuperAdminEmployee;

#[derive(Deserialize)]
pub struct PeriodQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
}

#[derive(Serialize)]
pub struct CategoryLine {
    category: String,
    #[serde(with = "rust_decimal::serde::float")]
    rate: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    subtotal: Decimal,
}

#[derive(Serialize)]
pub struct EmployeePay {
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    category_lines: Vec<CategoryLine>,
    #[serde(with = "rust_decimal::serde::float")]
    unpaid_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    coaching_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    total_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    total_pay: Decimal,
}

// GET /admin/pay?period_start=…&period_end=… — computed pay per employee. Super-admin only.
pub async fn list_pay(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeePay>>, (StatusCode, String)> {
    // Ingredient 1: every employee (so nobody is missing from the sheet).
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 2: hours grouped by employee AND category for the period.
    let hour_rows = sqlx::query!(
        r#"
        SELECT employee_id, category, SUM(hours) AS "hours!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2
        GROUP BY employee_id, category
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 3: every rate.
    let rate_rows = sqlx::query!(
        r#"SELECT employee_id, label, amount FROM employee_rates"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Ingredient 4: the dollar buckets for the period.
    let bucket_rows = sqlx::query!(
        r#"
        SELECT employee_id, other_earn, competition_earn, coaching_earn
        FROM period_category_totals
        WHERE period_start = $1 AND period_end = $2
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Build lookup maps for fast in-memory assembly.
    let mut rate_map: HashMap<(i64, String), Decimal> = HashMap::new();
    for r in &rate_rows {
        rate_map.insert((r.employee_id, r.label.clone()), r.amount);
    }

    let mut hours_map: HashMap<i64, Vec<(Option<String>, Decimal)>> = HashMap::new();
    for h in &hour_rows {
        hours_map.entry(h.employee_id).or_default().push((h.category.clone(), h.hours));
    }

    let mut bucket_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for b in &bucket_rows {
        bucket_map.insert(b.employee_id, (b.other_earn, b.competition_earn, b.coaching_earn));
    }

    // Assemble one pay record per employee.
    let mut result = Vec::new();
    for emp in &employees {
        let mut category_lines = Vec::new();
        let mut unpaid_hours = Decimal::ZERO;
        let mut total_hours = Decimal::ZERO;
        let mut category_pay = Decimal::ZERO;

        if let Some(hours) = hours_map.get(&emp.id) {
            for (category, hrs) in hours {
                total_hours += *hrs;
                match category {
                    // Logged under a real category…
                    Some(cat) => match rate_map.get(&(emp.id, cat.clone())) {
                        // …and a rate exists → a paid line.
                        Some(rate) => {
                            let subtotal = (*hrs * *rate).round_dp(2);
                            category_pay += subtotal;
                            category_lines.push(CategoryLine {
                                category: cat.clone(),
                                rate: *rate,
                                hours: *hrs,
                                subtotal,
                            });
                        }
                        // …but no rate → unpaid, needs a rate.
                        None => unpaid_hours += *hrs,
                    },
                    // Blank category → unpaid.
                    None => unpaid_hours += *hrs,
                }
            }
        }

        let (other, competition, coaching) = bucket_map
            .get(&emp.id)
            .copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

        category_lines.sort_by(|a, b| a.category.cmp(&b.category));
        let total_pay = category_pay + other + competition + coaching;

        result.push(EmployeePay {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            category_lines,
            unpaid_hours,
            other_earn: other,
            competition_earn: competition,
            coaching_earn: coaching,
            total_hours,
            total_pay,
        });
    }

    Ok(Json(result))
}