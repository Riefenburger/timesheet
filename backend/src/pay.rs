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
    rate: Decimal,       // 0 when no rate is set
    needs_rate: bool,    // true = category has hours but no rate
    #[serde(with = "rust_decimal::serde::float")]
    regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    sick_hours: Decimal,
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
    pay_method: String,
    pay_lines: Vec<PayLine>,
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
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number, pay_method FROM employees ORDER BY name"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Hours summed per employee, category, type — from entries.
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
        let mut pay_lines = Vec::new();
        let mut unpaid_hours = Decimal::ZERO;
        let mut total_hours = Decimal::ZERO;
        let mut category_pay = Decimal::ZERO;

        if let Some(cats) = cat_map.get(&emp.id) {
            let mut keys: Vec<&String> = cats.keys().collect();
            keys.sort();
            for cat in keys {
                let (reg, ot, sick) = cats[cat];
                let line_hours = reg + ot + sick;
                total_hours += line_hours;

                match rate_map.get(&(emp.id, cat.clone())) {
                    Some(rate) => {
                        let subtotal = (line_hours * *rate).round_dp(2);
                        category_pay += subtotal;
                        pay_lines.push(PayLine {
                            category: cat.clone(),
                            rate: *rate,
                            needs_rate: false,
                            regular_hours: reg,
                            overtime_hours: ot,
                            sick_hours: sick,
                            hours: line_hours,
                            subtotal,
                        });
                    }
                    None => {
                        // Category has hours but no rate — its own flagged row.
                        unpaid_hours += line_hours;
                        pay_lines.push(PayLine {
                            category: cat.clone(),
                            rate: Decimal::ZERO,
                            needs_rate: true,
                            regular_hours: reg,
                            overtime_hours: ot,
                            sick_hours: sick,
                            hours: line_hours,
                            subtotal: Decimal::ZERO,
                        });
                    }
                }
            }
        }

        // Add zero-hour rows for any rate the employee has but didn't log hours in.
        for r in &rate_rows {
            if r.employee_id != emp.id { continue; }
            let already = pay_lines.iter().any(|l| l.category == r.label);
            if !already {
                pay_lines.push(PayLine {
                    category: r.label.clone(),
                    rate: r.amount,
                    needs_rate: false,
                    regular_hours: Decimal::ZERO,
                    overtime_hours: Decimal::ZERO,
                    sick_hours: Decimal::ZERO,
                    hours: Decimal::ZERO,
                    subtotal: Decimal::ZERO,
                });
            }
        }

        let (other, competition, coaching) = dollar_map
            .get(&emp.id).copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));

        let total_pay = category_pay + other + competition + coaching;

        pay_lines.sort_by(|a, b| a.category.cmp(&b.category));

        result.push(EmployeePay {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            pay_method: emp.pay_method.clone(),
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