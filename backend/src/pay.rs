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
pub struct PayLine {
    category: String,
    #[serde(with = "rust_decimal::serde::float")]
    rate: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    hours: Decimal,     // reg + ot + sick
    #[serde(with = "rust_decimal::serde::float")]
    subtotal: Decimal,  // hours * rate
}

#[derive(Serialize)]
pub struct EmployeePay {
    employee_id: i64,
    employee_name: String,
    employee_number: String,
    pay_lines: Vec<PayLine>,
    #[serde(with = "rust_decimal::serde::float")]
    unpaid_hours: Decimal, // hours in categories with no rate (incl. uncategorized)
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
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Saved per-category hours for the period.
    let hours = sqlx::query!(
        r#"
        SELECT employee_id, category, regular_hours, overtime_hours, sick_hours
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2
        "#,
        period.period_start, period.period_end,
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rate_rows = sqlx::query!(
        r#"SELECT employee_id, label, amount FROM employee_rates"#
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

    let mut rate_map: HashMap<(i64, String), Decimal> = HashMap::new();
    for r in &rate_rows {
        rate_map.insert((r.employee_id, r.label.clone()), r.amount);
    }

    let mut hours_map: HashMap<i64, Vec<(String, Decimal, Decimal, Decimal)>> = HashMap::new();
    for h in &hours {
        hours_map.entry(h.employee_id).or_default().push((
            h.category.clone(), h.regular_hours, h.overtime_hours, h.sick_hours,
        ));
    }

    let mut dollar_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for d in &dollars {
        dollar_map.insert(d.employee_id, (d.other_earn, d.competition_earn, d.coaching_earn));
    }

    let mut result = Vec::new();
    for emp in &employees {
        let mut pay_lines = Vec::new();
        let mut unpaid_hours = Decimal::ZERO;
        let mut total_hours = Decimal::ZERO;
        let mut category_pay = Decimal::ZERO;

        if let Some(rows) = hours_map.get(&emp.id) {
            for (cat, reg, ot, sick) in rows {
                let line_hours = *reg + *ot + *sick;
                total_hours += line_hours;

                match rate_map.get(&(emp.id, cat.clone())) {
                    Some(rate) => {
                        let subtotal = (line_hours * *rate).round_dp(2);
                        category_pay += subtotal;
                        pay_lines.push(PayLine {
                            category: cat.clone(),
                            rate: *rate,
                            regular_hours: *reg,
                            overtime_hours: *ot,
                            sick_hours: *sick,
                            hours: line_hours,
                            subtotal,
                        });
                    }
                    // No rate (includes 'uncategorized') → unpaid.
                    None => unpaid_hours += line_hours,
                }
            }
        }

        let (other, competition, coaching) = dollar_map
            .get(&emp.id).copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

        pay_lines.sort_by(|a, b| a.category.cmp(&b.category));
        let total_pay = category_pay + other + competition + coaching;

        result.push(EmployeePay {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            pay_lines,
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