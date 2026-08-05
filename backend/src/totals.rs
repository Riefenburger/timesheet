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

use chrono::Datelike;

// One entry's minimal shape for the overtime walk.
pub struct OvertimeEntry {
    pub entry_date: NaiveDate,
    pub hours: Decimal,
    pub category: String,
    pub is_private: bool,
}

// The regular/overtime split for one entry.
#[derive(Debug, PartialEq)]
pub struct OvertimeSplit {
    pub regular: Decimal,
    pub overtime: Decimal,
}

#[derive(Serialize, Clone)]
pub struct LumpSumRow {
    category: String,
    #[serde(with = "rust_decimal::serde::float")]
    amount: Decimal,       // computed dollars for this lump-sum category this period
    entry_count: i64,
}

// The Sunday that starts the Sun–Sat week containing `d`.
fn week_start(d: NaiveDate) -> NaiveDate {
    // chrono: Mon=0 .. Sun=6 via num_days_from_monday; we want days since Sunday.
    let days_since_sunday = (d.weekday().num_days_from_sunday()) as i64;
    d - chrono::Duration::days(days_since_sunday)
}

// Walk each Sun–Sat week chronologically (regular before private within a day),
// splitting each entry's hours at the 40-hour line. Returns splits in input order.
pub fn compute_overtime(entries: &[OvertimeEntry]) -> Vec<OvertimeSplit> {
    let threshold = Decimal::from(40);
    let mut results: Vec<OvertimeSplit> =
        (0..entries.len()).map(|_| OvertimeSplit { regular: Decimal::ZERO, overtime: Decimal::ZERO }).collect();

    // Group entry indices by week-start.
    let mut weeks: HashMap<NaiveDate, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        weeks.entry(week_start(e.entry_date)).or_default().push(i);
    }

    for (_wk, mut idxs) in weeks {
        // Order: by date, then regular(false) before private(true), then original index.
        idxs.sort_by(|&a, &b| {
            let ea = &entries[a];
            let eb = &entries[b];
            ea.entry_date.cmp(&eb.entry_date)
                .then(ea.is_private.cmp(&eb.is_private))
                .then(a.cmp(&b))
        });

        let mut cumulative = Decimal::ZERO;
        for i in idxs {
            let h = entries[i].hours;
            let start = cumulative;
            let end = cumulative + h;
            let (reg, ot) = if end <= threshold {
                (h, Decimal::ZERO)
            } else if start >= threshold {
                (Decimal::ZERO, h)
            } else {
                (threshold - start, end - threshold)
            };
            results[i] = OvertimeSplit { regular: reg, overtime: ot };
            cumulative = end;
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> NaiveDate { NaiveDate::from_str(s).unwrap() }
    fn dec(n: &str) -> Decimal { Decimal::from_str(n).unwrap() }
    fn e(date: &str, hours: &str, cat: &str, priv_: bool) -> OvertimeEntry {
        OvertimeEntry { entry_date: d(date), hours: dec(hours), category: cat.to_string(), is_private: priv_ }
    }

    #[test]
    fn under_40_all_regular() {
        let entries = vec![
            e("2026-07-06", "8", "teaching", false),
            e("2026-07-07", "8", "teaching", false),
            e("2026-07-08", "8", "office", false),
        ];
        let r = compute_overtime(&entries);
        for split in &r { assert_eq!(split.overtime, Decimal::ZERO); }
        assert_eq!(r[0].regular, dec("8"));
    }

    #[test]
    fn exactly_40_then_overtime() {
        let entries = vec![
            e("2026-07-06", "10", "teaching", false),
            e("2026-07-07", "10", "teaching", false),
            e("2026-07-08", "10", "teaching", false),
            e("2026-07-09", "10", "office", false),   // ends exactly at 40
            e("2026-07-10", "5", "office", false),    // all OT
        ];
        let r = compute_overtime(&entries);
        assert_eq!(r[3], OvertimeSplit { regular: dec("10"), overtime: Decimal::ZERO });
        assert_eq!(r[4], OvertimeSplit { regular: Decimal::ZERO, overtime: dec("5") });
    }

    #[test]
    fn straddles_the_line() {
        let entries = vec![
            e("2026-07-06", "38", "teaching", false),
            e("2026-07-07", "4", "office", false),  // 2 reg + 2 OT
        ];
        let r = compute_overtime(&entries);
        assert_eq!(r[1], OvertimeSplit { regular: dec("2"), overtime: dec("2") });
    }

    #[test]
    fn private_after_regular_same_day() {
        let entries = vec![
            e("2026-07-06", "38", "teaching", false),
            e("2026-07-07", "1", "private", true),   // private, listed first but counts last
            e("2026-07-07", "3", "office", false),   // regular, counts first: 38->41 (2reg+1ot)
        ];
        let r = compute_overtime(&entries);
        assert_eq!(r[2], OvertimeSplit { regular: dec("2"), overtime: dec("1") }); // office
        assert_eq!(r[1], OvertimeSplit { regular: Decimal::ZERO, overtime: dec("1") }); // private all OT
    }

    #[test]
    fn week_straddles_two_periods() {
        let entries = vec![
            e("2026-07-13", "20", "teaching", false),  // period 1
            e("2026-07-14", "22", "teaching", false),  // period 1: 20 reg + 2 OT
            e("2026-07-16", "5", "office", false),     // period 2: all OT
        ];
        let r = compute_overtime(&entries);
        assert_eq!(r[0], OvertimeSplit { regular: dec("20"), overtime: Decimal::ZERO });
        assert_eq!(r[1], OvertimeSplit { regular: dec("20"), overtime: dec("2") });
        assert_eq!(r[2], OvertimeSplit { regular: Decimal::ZERO, overtime: dec("5") });
    }
}

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
             regular_hours, overtime_hours, sick_hours, admin_edited)
        VALUES ($1, $2, $3, $4, $5, $6, $7, true)
        ON CONFLICT (employee_id, period_start, period_end, category, session_duration)
        DO UPDATE SET
            regular_hours  = EXCLUDED.regular_hours,
            overtime_hours = EXCLUDED.overtime_hours,
            sick_hours     = EXCLUDED.sick_hours,
            admin_edited   = true
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
             session_duration, session_count, admin_edited)
        VALUES ($1, $2, $3, 'private', $4, $5, true)
        ON CONFLICT (employee_id, period_start, period_end, category, session_duration)
        DO UPDATE SET session_count = EXCLUDED.session_count, admin_edited = true
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
    pub category: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub overtime_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub sick_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub logged_hours: Decimal,
    pub admin_edited: bool,
}

#[derive(Serialize)]
pub struct PrivateRow {
    pub session_duration: i32,
    pub session_count: i32,
    pub logged_count: i32,
    pub admin_edited: bool,
}

#[derive(Serialize, Clone)]
pub struct RateEntry {
    pub label: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub amount: Decimal,
}

#[derive(Serialize)]
pub struct EmployeeTotals {
    pub employee_id: i64,
    pub employee_name: String,
    pub employee_number: String,
    pub pay_method: String,
    pub pay_frequency: String,
    pub categories: Vec<CategoryRow>,
    pub private_sessions: Vec<PrivateRow>,
    #[serde(with = "rust_decimal::serde::float")]
    pub other_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub competition_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub coaching_earn: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub lump_sum_earn: Decimal,
    pub lump_sums: Vec<LumpSumRow>,
    #[serde(with = "rust_decimal::serde::float")]
    pub private_regular_hours: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub private_overtime_hours: Decimal,
    pub rates: Vec<RateEntry>,
}

// GET /admin/totals?period_start=…&period_end=… — thin HTTP wrapper.
pub async fn list_totals(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Query(period): Query<PeriodQuery>,
) -> Result<Json<Vec<EmployeeTotals>>, (StatusCode, String)> {
    let result = compute_totals(&pool, period.period_start, period.period_end).await?;
    Ok(Json(result))
}

// The actual computation, reusable by the Excel export. Takes plain args.
pub async fn compute_totals(
    pool: &PgPool,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> Result<Vec<EmployeeTotals>, (StatusCode, String)> {
    // All employees.
    let employees = sqlx::query!(
        r#"SELECT id, name, employee_number, pay_method, pay_frequency FROM employees ORDER BY name"#
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Stored (admin_edited) normal category rows.
    let stored_cats = sqlx::query!(
        r#"
        SELECT employee_id, category, regular_hours, overtime_hours, sick_hours
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2
          AND session_duration IS NULL AND admin_edited = true
        "#,
        period_start, period_end,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Stored (admin_edited) private rows.
    let stored_privates = sqlx::query!(
        r#"
        SELECT employee_id, session_duration AS "session_duration!", session_count AS "session_count!"
        FROM category_hours
        WHERE period_start = $1 AND period_end = $2
          AND session_duration IS NOT NULL AND admin_edited = true
        "#,
        period_start, period_end,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Dollar buckets.
    let dollars = sqlx::query!(
        r#"
        SELECT employee_id, other_earn, competition_earn, coaching_earn
        FROM period_dollar_totals
        WHERE period_start = $1 AND period_end = $2
        "#,
        period_start, period_end,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // All employee rates (ordered by id so the export's Hourly/Rate 2/… is stable).
    let rates = sqlx::query!(
        r#"SELECT employee_id, label, amount FROM employee_rates ORDER BY id"#
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Widen ±6 days for straddling Sun–Sat weeks. Sick excluded from the walk.
    let window_start = period_start - chrono::Duration::days(6);
    let window_end = period_end + chrono::Duration::days(6);
    let worked = sqlx::query!(
        r#"
        SELECT employee_id, entry_date,
               hours,
               COALESCE(category, 'uncategorized') AS "category!",
               (session_duration IS NOT NULL) AS "is_private!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2 AND type != 'sick'
        "#,
        window_start, window_end,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Logged private counts per duration, within the period (for mismatch).
    let logged_privates = sqlx::query!(
        r#"
        SELECT employee_id, session_duration AS "session_duration!", SUM(session_count)::int AS "count!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2 AND session_duration IS NOT NULL
        GROUP BY employee_id, session_duration
        "#,
        period_start, period_end,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Which category names are lump-sum.
    let lump_sum_cats: Vec<String> = sqlx::query_scalar!(
        r#"SELECT name FROM categories WHERE is_lump_sum = true"#
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Lump-sum entry counts per (employee, category) within the period.
    let lump_entries = sqlx::query!(
        r#"
        SELECT employee_id, category AS "category!", COUNT(*) AS "count!"
        FROM time_entries
        WHERE entry_date BETWEEN $1 AND $2 AND category = ANY($3)
        GROUP BY employee_id, category
        "#,
        period_start, period_end, &lump_sum_cats,
    ).fetch_all(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // --- Index stored/dollar data ---
    let mut stored_map: HashMap<i64, HashMap<String, (Decimal, Decimal, Decimal)>> = HashMap::new();
    for s in &stored_cats {
        stored_map.entry(s.employee_id).or_default()
            .insert(s.category.clone(), (s.regular_hours, s.overtime_hours, s.sick_hours));
    }
    let mut stored_priv_map: HashMap<i64, HashMap<i32, i32>> = HashMap::new();
    for p in &stored_privates {
        stored_priv_map.entry(p.employee_id).or_default().insert(p.session_duration, p.session_count);
    }
    let mut logged_priv_map: HashMap<i64, HashMap<i32, i32>> = HashMap::new();
    for p in &logged_privates {
        logged_priv_map.entry(p.employee_id).or_default().insert(p.session_duration, p.count);
    }
    let mut dollar_map: HashMap<i64, (Decimal, Decimal, Decimal)> = HashMap::new();
    for d in &dollars {
        dollar_map.insert(d.employee_id, (d.other_earn, d.competition_earn, d.coaching_earn));
    }
    let mut rate_map: HashMap<i64, Vec<RateEntry>> = HashMap::new();
    for r in &rates {
        rate_map.entry(r.employee_id).or_default()
            .push(RateEntry { label: r.label.clone(), amount: r.amount });
    }
    let mut lump_dollars: HashMap<i64, Decimal> = HashMap::new();
    let mut lump_rows: HashMap<i64, Vec<LumpSumRow>> = HashMap::new();
    for le in &lump_entries {
        let amount = rate_map.get(&le.employee_id)
            .and_then(|rates| rates.iter().find(|r| r.label == le.category))
            .map(|r| r.amount)
            .unwrap_or(Decimal::ZERO);
        let line_total = amount * Decimal::from(le.count);
        *lump_dollars.entry(le.employee_id).or_insert(Decimal::ZERO) += line_total;
        lump_rows.entry(le.employee_id).or_default().push(LumpSumRow {
            category: le.category.clone(),
            amount: line_total,
            entry_count: le.count,
        });
    }
    let mut worked_by_emp: HashMap<i64, Vec<OvertimeEntry>> = HashMap::new();
    for w in &worked {
        worked_by_emp.entry(w.employee_id).or_default().push(OvertimeEntry {
            entry_date: w.entry_date,
            hours: w.hours,
            category: w.category.clone(),
            is_private: w.is_private,
        });
    }
    // --- Assemble per employee ---
    let mut result = Vec::new();
    for emp in &employees {
        let mut computed: HashMap<String, (Decimal, Decimal)> = HashMap::new();
        let mut logged_cat: HashMap<String, Decimal> = HashMap::new();
        if let Some(entries) = worked_by_emp.get(&emp.id) {
            let splits = compute_overtime(entries);
            for (entry, split) in entries.iter().zip(splits.iter()) {
                if entry.entry_date >= period_start && entry.entry_date <= period_end {
                    let c = computed.entry(entry.category.clone()).or_insert((Decimal::ZERO, Decimal::ZERO));
                    c.0 += split.regular;
                    c.1 += split.overtime;
                    *logged_cat.entry(entry.category.clone()).or_insert(Decimal::ZERO) += entry.hours;
                }
            }
        }
        let mut cat_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for k in computed.keys() { cat_names.insert(k.clone()); }
        if let Some(m) = stored_map.get(&emp.id) { for k in m.keys() { cat_names.insert(k.clone()); } }
        let mut categories = Vec::new();
        for cat in &cat_names {
            if cat == "private" { continue; }
            if lump_sum_cats.contains(cat) { continue; }
            let logged = logged_cat.get(cat).copied().unwrap_or(Decimal::ZERO);
            if let Some((reg, ot, sick)) = stored_map.get(&emp.id).and_then(|m| m.get(cat)).copied() {
                categories.push(CategoryRow {
                    category: cat.clone(),
                    regular_hours: reg, overtime_hours: ot, sick_hours: sick,
                    logged_hours: logged, admin_edited: true,
                });
            } else {
                let (reg, ot) = computed.get(cat).copied().unwrap_or((Decimal::ZERO, Decimal::ZERO));
                categories.push(CategoryRow {
                    category: cat.clone(),
                    regular_hours: reg, overtime_hours: ot, sick_hours: Decimal::ZERO,
                    logged_hours: logged, admin_edited: false,
                });
            }
        }
        let mut durations: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
        if let Some(m) = logged_priv_map.get(&emp.id) { for k in m.keys() { durations.insert(*k); } }
        if let Some(m) = stored_priv_map.get(&emp.id) { for k in m.keys() { durations.insert(*k); } }
        let mut private_sessions = Vec::new();
        for dur in &durations {
            let logged = logged_priv_map.get(&emp.id).and_then(|m| m.get(dur)).copied().unwrap_or(0);
            if let Some(count) = stored_priv_map.get(&emp.id).and_then(|m| m.get(dur)).copied() {
                private_sessions.push(PrivateRow {
                    session_duration: *dur, session_count: count, logged_count: logged, admin_edited: true,
                });
            } else {
                private_sessions.push(PrivateRow {
                    session_duration: *dur, session_count: logged, logged_count: logged, admin_edited: false,
                });
            }
        }
        let (other, competition, coaching) = dollar_map.get(&emp.id).copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO, Decimal::ZERO));
        let lump = lump_dollars.get(&emp.id).copied().unwrap_or(Decimal::ZERO);
        let (priv_reg, priv_ot) = computed.get("private").copied()
            .unwrap_or((Decimal::ZERO, Decimal::ZERO));
        result.push(EmployeeTotals {
            employee_id: emp.id,
            employee_name: emp.name.clone(),
            employee_number: emp.employee_number.clone(),
            pay_method: emp.pay_method.clone(),
            pay_frequency: emp.pay_frequency.clone(),
            categories,
            private_sessions,
            other_earn: other, competition_earn: competition, coaching_earn: coaching,
            lump_sum_earn: lump,
            lump_sums: lump_rows.get(&emp.id).cloned().unwrap_or_default(),
            private_regular_hours: priv_reg,
            private_overtime_hours: priv_ot,
            rates: rate_map.get(&emp.id).cloned().unwrap_or_default(),
        });
    }
    Ok(result)
}

#[derive(Deserialize)]
pub struct RevertInput {
    employee_id: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    category: String,
    #[serde(default)]
    session_duration: Option<i32>,
}

// POST /admin/category-hours/revert — drop an admin override so the row goes
// back to being computed from entries. Admin-gated.
pub async fn revert_category_hours(
    State(pool): State<PgPool>,
    _admin: AdminEmployee,
    Json(payload): Json<RevertInput>,
) -> Result<StatusCode, (StatusCode, String)> {
    sqlx::query!(
        r#"
        DELETE FROM category_hours
        WHERE employee_id = $1 AND period_start = $2 AND period_end = $3
          AND category = $4
          AND session_duration IS NOT DISTINCT FROM $5
        "#,
        payload.employee_id,
        payload.period_start,
        payload.period_end,
        payload.category,
        payload.session_duration,
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}