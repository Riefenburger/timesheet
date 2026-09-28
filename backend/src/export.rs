use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use rust_xlsxwriter::{Workbook, Format, FormatAlign, FormatBorder};

use crate::auth::SuperAdminEmployee;
use crate::totals::{compute_totals, CategoryRow, PrivateRow, RateEntry};
use chrono::NaiveDate;

fn as_f64(d: Decimal) -> f64 { d.to_f64().unwrap_or(0.0) }

// --- Per-employee block model ---------------------------------------------

/// What the numeric columns hold for one row of an employee's block.
#[derive(Debug, Clone, PartialEq)]
pub enum RowUnits {
    /// Hours worked at this row's rate. All-zero renders as blank cells.
    Hours { regular: Decimal, overtime: Decimal },
    /// A private-lesson row: a session COUNT, shown in the Regular column.
    /// Tracked apart from Hours so reconciliation never reads 4 sessions as 4 hours.
    Sessions(i32),
    /// Nothing numeric on this row.
    Empty,
}

/// One row of an employee's block: the column-A text plus what goes in the
/// numeric columns.
#[derive(Debug, Clone, PartialEq)]
pub struct RateRow {
    pub label: String,
    pub units: RowUnits,
}

/// How a row's rate is introduced in column A.
enum RowKind { Hourly, PerSession, Salary }

/// Build one employee's block, routing each category's hours onto the row of the
/// rate they were logged under (rate `label` == category name, matched
/// case-insensitively and trimmed).
///
/// RECONCILIATION IS THE POINT. Because hours are now routed to specific rows
/// rather than summed blindly, anything that fails to match a row would vanish
/// from the sheet and underpay someone. So: categories with hours that no rate
/// claims get an explicit "(no rate)" row, private durations with sessions but
/// no rate likewise, and any residue that still escapes lands on an
/// "(unassigned)" row. Nothing is ever silently dropped.
///
/// PRIVATE LESSONS are counted, not clocked: their rows show a session COUNT in
/// the Regular column, with "(per session)" in the label. The sheet does no
/// arithmetic — a person reads it and runs payroll by hand — so the Regular
/// column deliberately mixes hours (30.00) and session counts (4) on different
/// rows, and the row label is what tells the reader which is which. Rates are
/// shown for that reader's reference only.
///
/// Two consequences of going count-based for privates, both deliberate:
/// * The export/screen disagreement over private hours goes away: both sides now
///   key off the same per-duration `session_count`, so an admin's edit on the
///   totals screen flows into the export instead of the export quietly using
///   hours from the logged entries. (The screen's Regular column still *adds*
///   private hours, while this sheet keeps private hours out of Regular
///   entirely. That difference is by design, not drift.)
/// * The dropped `private_overtime_hours` is moot: the export no longer uses the
///   private regular/OT hour split at all, in either column.
pub fn build_block(
    is_salaried: bool,
    salary: Option<Decimal>,
    categories: &[CategoryRow],
    rates: &[RateEntry],
    private_sessions: &[PrivateRow],
    lump_sum_cats: &[String],
) -> Vec<RateRow> {
    let norm = |s: &str| s.trim().to_lowercase();

    // Everything the employee-level totals say must appear somewhere in the block.
    let expected_regular: Decimal = categories.iter().map(|c| c.regular_hours).sum();
    let expected_overtime: Decimal = categories.iter().map(|c| c.overtime_hours).sum();
    let expected_sessions: i32 = private_sessions.iter()
        .map(|p| p.session_count).filter(|n| *n > 0).sum();

    let mut numbered: Vec<(RowKind, String, RowUnits)> = Vec::new();
    let mut claimed: HashSet<String> = HashSet::new();
    let mut orphans: Vec<RateRow> = Vec::new();

    if is_salaried {
        // Salaried staff have no rate rows to distribute across, so their hours
        // stay on the block's single row, as before.
        let amount = salary.map(|v| format!("{:.2}", as_f64(v)))
            .unwrap_or_else(|| "0.00".to_string());
        numbered.push((RowKind::Salary, amount, RowUnits::Hours {
            regular: expected_regular,
            overtime: expected_overtime,
        }));
        for c in categories { claimed.insert(norm(&c.category)); }
    } else {
        // Normal rates in employee_rates.id order (they arrive ordered), minus
        // private and lump-sum labels — lump-sum dollars ride in Other $$.
        for rt in rates.iter().filter(|rt| {
            !rt.label.starts_with("private_")
                && !lump_sum_cats.iter().any(|l| norm(l) == norm(&rt.label))
        }) {
            let key = norm(&rt.label);
            // A second rate with the same label must not double-count the hours.
            let hit = if claimed.contains(&key) { None }
                else { categories.iter().find(|c| norm(&c.category) == key) };
            let (regular, overtime) = match hit {
                Some(c) => { claimed.insert(key); (c.regular_hours, c.overtime_hours) }
                None => (Decimal::ZERO, Decimal::ZERO),
            };
            numbered.push((RowKind::Hourly, format!("{:.4}", as_f64(rt.amount)),
                RowUnits::Hours { regular, overtime }));
        }
    }

    // Private durations that actually had sessions this period, ascending.
    let mut active: Vec<&PrivateRow> = private_sessions.iter()
        .filter(|p| p.session_count > 0).collect();
    active.sort_by_key(|p| p.session_duration);
    for p in active {
        let label = format!("private_{}", p.session_duration);
        match rates.iter().find(|rt| norm(&rt.label) == label) {
            Some(rt) => numbered.push((RowKind::PerSession, format!("{:.2}", as_f64(rt.amount)),
                RowUnits::Sessions(p.session_count))),
            // Sessions at a duration this employee has no rate for: keep them visible.
            None => orphans.push(RateRow {
                label: format!("private {} min (no rate)", p.session_duration),
                units: RowUnits::Sessions(p.session_count),
            }),
        }
    }

    // Categories with hours that no rate row claimed.
    for c in categories {
        if claimed.contains(&norm(&c.category)) { continue; }
        if c.regular_hours == Decimal::ZERO && c.overtime_hours == Decimal::ZERO { continue; }
        orphans.push(RateRow {
            label: format!("{} (no rate)", c.category),
            units: RowUnits::Hours { regular: c.regular_hours, overtime: c.overtime_hours },
        });
    }

    // Normal rates run Hourly / Rate 2 / Rate 3…; private rows carry their own
    // "Private Rate N" sequence so the two numberings never interleave.
    let mut rows: Vec<RateRow> = Vec::new();
    let mut normal_n = 0usize;
    let mut private_n = 0usize;
    for (kind, amount, units) in numbered {
        let label = match kind {
            RowKind::Salary => format!("Salary: {}", amount),
            RowKind::Hourly => {
                normal_n += 1;
                if normal_n == 1 { format!("Hourly: {}", amount) }
                else { format!("Rate {}: {}", normal_n, amount) }
            }
            RowKind::PerSession => {
                private_n += 1;
                format!("Private Rate {}: {} (per session)", private_n, amount)
            }
        };
        rows.push(RateRow { label, units });
    }
    rows.extend(orphans);

    // --- Reconciliation ---
    // Hours rows must account for every category hour, and private rows for
    // every session. Sessions are deliberately NOT hours, so they reconcile
    // separately and are never compared against the hour totals — otherwise
    // count-based private rows would false-alarm here.
    let mut got_regular = Decimal::ZERO;
    let mut got_overtime = Decimal::ZERO;
    let mut got_sessions = 0i32;
    for r in &rows {
        match &r.units {
            RowUnits::Hours { regular, overtime } => {
                got_regular += *regular;
                got_overtime += *overtime;
            }
            RowUnits::Sessions(n) => got_sessions += *n,
            RowUnits::Empty => {}
        }
    }
    let miss_regular = expected_regular - got_regular;
    let miss_overtime = expected_overtime - got_overtime;
    let miss_sessions = expected_sessions - got_sessions;
    if miss_regular != Decimal::ZERO || miss_overtime != Decimal::ZERO {
        rows.push(RateRow {
            label: "(unassigned hours)".to_string(),
            units: RowUnits::Hours { regular: miss_regular, overtime: miss_overtime },
        });
    }
    if miss_sessions != 0 {
        rows.push(RateRow {
            label: "(unassigned sessions)".to_string(),
            units: RowUnits::Sessions(miss_sessions),
        });
    }

    // Every employee needs at least one row so their block renders.
    if rows.is_empty() {
        rows.push(RateRow { label: String::new(), units: RowUnits::Empty });
    }
    rows
}

#[derive(Deserialize)]
pub struct ExportQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
    // Optional: the client fills Run Number in on the sheet, so it is normally
    // absent and the header cell is left blank.
    #[serde(default)]
    run_number: Option<String>,
    // Filters mirroring the super-totals view. 'all' frequency = no freq filter.
    #[serde(default)]
    frequency: Option<String>,   // 'all' | 'weekly' | 'bimonthly' | 'monthly'
    #[serde(default)]
    pay_method: Option<String>,  // 'both' | 'payroll' | 'check'
}

// GET /admin/totals/export — build the isolved-style payroll timesheet .xlsx
// for the current view (period + filters). Super-admin only.
pub async fn export_totals(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Query(q): Query<ExportQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // 1. Compute all totals for the period.
    let all_totals = compute_totals(&pool, q.period_start, q.period_end).await?;

    // 2. Fetch salary info + which categories are lump-sum (to exclude from rates).
    let salary_rows = sqlx::query!(
        r#"SELECT id, is_salaried, salary FROM employees"#
    ).fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut salary_map: HashMap<i64, (bool, Option<Decimal>)> = HashMap::new();
    for s in &salary_rows {
        salary_map.insert(s.id, (s.is_salaried, s.salary));
    }
    let lump_sum_cats: Vec<String> = sqlx::query_scalar!(
        r#"SELECT name FROM categories WHERE is_lump_sum = true"#
    ).fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. Apply the same filters as the super-totals view.
    let freq = q.frequency.as_deref().unwrap_or("all");
    let paym = q.pay_method.as_deref().unwrap_or("both");
    let employees: Vec<_> = all_totals.into_iter().filter(|e| {
        (freq == "all" || e.pay_frequency == freq)
            && (paym == "both" || e.pay_method == paym)
    }).collect();

    // 4. Build the workbook.
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();

    // --- Formats ---
    let title_fmt = Format::new().set_bold().set_align(FormatAlign::Center).set_font_size(12.0);
    let label_fmt = Format::new().set_bold();
    let hdr_fmt = Format::new().set_bold().set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Left);
    let num_fmt = Format::new().set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Right).set_num_format("0.00");
    // Session counts share the Regular column with hours, so an integer format
    // keeps them visually distinct from the 2-decimal hour figures.
    let count_fmt = Format::new().set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Right).set_num_format("0");
    let empinfo_fmt = Format::new().set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Top).set_text_wrap();
    let cell_fmt = Format::new().set_border(FormatBorder::Thin);
    let fmt_date = |d: NaiveDate| d.format("%-m/%-d/%Y").to_string();

    // --- Header block ---
    sheet.merge_range(0, 0, 0, 6, "PAYROLL TIMESHEET", &title_fmt)
        .map_err(xerr)?;
    sheet.merge_range(1, 0, 1, 6, "Fishback Studio Of Dance LLC", &title_fmt)
        .map_err(xerr)?;

    // Left metadata column (labels + values), rows 3..
    let meta: Vec<(&str, String)> = vec![
        ("Client ID:", "C595 - Fishback Studio Of Dance LLC".to_string()),
        ("Pay Group:", "Semi-Monthly".to_string()),
        ("Check Date:", String::new()),
        ("Run Date:", String::new()),
        ("Run Number:", q.run_number.clone().unwrap_or_default()),
        ("Period Begin Date:", fmt_date(q.period_start)),
        ("Period End Date:", fmt_date(q.period_end)),
        ("Pay Period:", String::new()),
        ("Payroll Type:", "Regular Payroll".to_string()),
    ];
    let mut r = 3u32;
    for (label, value) in &meta {
        sheet.write_with_format(r, 0, *label, &label_fmt).map_err(xerr)?;
        sheet.write(r, 1, value.as_str()).map_err(xerr)?;
        r += 1;
    }

    // --- Column headers ---
    let header_row = r + 1;
    let headers = ["Employee Information", "Regular Hours", "Overtime Hours",
        "Other $$", "Competition", "Coaching", "Sick Hours"];
    for (c, h) in headers.iter().enumerate() {
        sheet.write_with_format(header_row, c as u16, *h, &hdr_fmt).map_err(xerr)?;
    }

    // Column widths.
    sheet.set_column_width(0, 34.0).map_err(xerr)?;
    for c in 1..=6u16 { sheet.set_column_width(c, 13.0).map_err(xerr)?; }

    // --- Per-employee rows ---
    // Each employee occupies a block of rows: one per rate row, plus any
    // "(no rate)" rows. Hours sit on the row of the rate they were logged under;
    // the dollar columns and Sick are employee-level and stay on the first row.
    let mut row = header_row + 1;
    for emp in &employees {
        let (is_salaried, salary) = salary_map.get(&emp.employee_id).copied()
            .unwrap_or((false, None));

        let block = build_block(
            is_salaried, salary,
            &emp.categories, &emp.rates, &emp.private_sessions,
            &lump_sum_cats,
        );

        let first = row;
        let last = first + block.len() as u32 - 1;

        // Column A: name + Emp# + the first row's rate share one wrapped cell.
        let mut head = vec![
            if emp.is_active {
                emp.employee_name.clone()
            } else {
                // Only inactive employees who still have hours/sessions in this
                // period reach the sheet at all — flag them so payroll knows.
                format!("{} (inactive)", emp.employee_name)
            },
            format!("Emp#: {}", emp.employee_number),
        ];
        if !block[0].label.is_empty() { head.push(block[0].label.clone()); }
        sheet.write_with_format(first, 0, head.join("\n").as_str(), &empinfo_fmt).map_err(xerr)?;
        sheet.set_row_height(first, 15.0 * head.len() as f64).map_err(xerr)?;
        // Remaining rows carry their own label.
        for (i, rr) in block.iter().enumerate().skip(1) {
            sheet.write_with_format(first + i as u32, 0, rr.label.as_str(), &cell_fmt).map_err(xerr)?;
        }

        // Regular (col 1) and Overtime (col 2), per row.
        for (i, rr) in block.iter().enumerate() {
            let rw = first + i as u32;
            match &rr.units {
                RowUnits::Hours { regular, overtime }
                    if *regular != Decimal::ZERO || *overtime != Decimal::ZERO =>
                {
                    sheet.write_with_format(rw, 1, as_f64(*regular), &num_fmt).map_err(xerr)?;
                    sheet.write_with_format(rw, 2, as_f64(*overtime), &num_fmt).map_err(xerr)?;
                }
                RowUnits::Sessions(n) => {
                    // A session count, not hours — the row label says "(per session)".
                    sheet.write_with_format(rw, 1, *n as f64, &count_fmt).map_err(xerr)?;
                    sheet.write_with_format(rw, 2, "", &cell_fmt).map_err(xerr)?;
                }
                // Rate rows with no hours this period stay blank.
                _ => {
                    sheet.write_with_format(rw, 1, "", &cell_fmt).map_err(xerr)?;
                    sheet.write_with_format(rw, 2, "", &cell_fmt).map_err(xerr)?;
                }
            }
        }

        // Employee-level columns: values on the first row, blanks below.
        let sick: Decimal = emp.categories.iter().map(|c| c.sick_hours).sum::<Decimal>();
        let other = emp.other_earn + emp.lump_sum_earn; // combined (typed + lump)
        let level = [other, emp.competition_earn, emp.coaching_earn, sick];
        for (c, v) in level.iter().enumerate() {
            let col = (c + 3) as u16;
            sheet.write_with_format(first, col, as_f64(*v), &num_fmt).map_err(xerr)?;
            for br in (first + 1)..=last {
                sheet.write_with_format(br, col, "", &cell_fmt).map_err(xerr)?;
            }
        }

        row = last + 1;
    }

    // 5. Serialize to bytes and return as a download.
    let buf = workbook.save_to_buffer().map_err(xerr)?;

    let filename = format!("payroll_timesheet_{}.xlsx", q.period_end);
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename)),
        ],
        buf,
    ))
}

fn xerr(e: rust_xlsxwriter::XlsxError) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("Excel error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(s: &str) -> Decimal { Decimal::from_str(s).unwrap() }

    fn cat(name: &str, reg: &str, ot: &str) -> CategoryRow {
        CategoryRow {
            category: name.to_string(),
            regular_hours: d(reg),
            overtime_hours: d(ot),
            sick_hours: Decimal::ZERO,
            logged_hours: d(reg),
            admin_edited: false,
        }
    }
    fn rate(label: &str, amount: &str) -> RateEntry {
        RateEntry { label: label.to_string(), amount: d(amount) }
    }
    fn sessions(duration: i32, count: i32) -> PrivateRow {
        PrivateRow {
            session_duration: duration,
            session_count: count,
            logged_count: count,
            admin_edited: false,
        }
    }
    fn hours(reg: &str, ot: &str) -> RowUnits {
        RowUnits::Hours { regular: d(reg), overtime: d(ot) }
    }
    fn labels(rows: &[RateRow]) -> Vec<&str> {
        rows.iter().map(|r| r.label.as_str()).collect()
    }
    // What the block actually accounts for, for the reconciliation assertions.
    fn accounted(rows: &[RateRow]) -> (Decimal, Decimal, i32) {
        let mut reg = Decimal::ZERO;
        let mut ot = Decimal::ZERO;
        let mut sess = 0;
        for r in rows {
            match &r.units {
                RowUnits::Hours { regular, overtime } => { reg += *regular; ot += *overtime; }
                RowUnits::Sessions(n) => sess += *n,
                RowUnits::Empty => {}
            }
        }
        (reg, ot, sess)
    }
    fn has_unassigned(rows: &[RateRow]) -> bool {
        rows.iter().any(|r| r.label.starts_with("(unassigned"))
    }

    #[test]
    fn hours_split_across_rate_rows() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "30", "2"), cat("office", "8", "0")],
            &[rate("teaching", "25"), rate("office", "18")],
            &[], &[],
        );
        assert_eq!(labels(&rows), vec!["Hourly: 25.0000", "Rate 2: 18.0000"]);
        assert_eq!(rows[0].units, hours("30", "2"));
        assert_eq!(rows[1].units, hours("8", "0"));
        assert!(!has_unassigned(&rows));
    }

    #[test]
    fn category_with_no_rate_gets_its_own_row() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "30", "0"), cat("office", "8", "1")],
            &[rate("teaching", "25")],
            &[], &[],
        );
        assert_eq!(labels(&rows), vec!["Hourly: 25.0000", "office (no rate)"]);
        assert_eq!(rows[1].units, hours("8", "1"));
        // Nothing dropped: the orphan's hours are still accounted for.
        assert_eq!(accounted(&rows), (d("38"), d("1"), 0));
        assert!(!has_unassigned(&rows));
    }

    #[test]
    fn uncategorized_hours_are_kept() {
        // Entries with a NULL category arrive as 'uncategorized' and never have a rate.
        let rows = build_block(false, None, &[cat("uncategorized", "5", "0")], &[], &[], &[]);
        assert_eq!(labels(&rows), vec!["uncategorized (no rate)"]);
        assert_eq!(accounted(&rows), (d("5"), Decimal::ZERO, 0));
    }

    #[test]
    fn rate_with_no_hours_this_period_is_blank() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "30", "2")],
            &[rate("teaching", "25"), rate("office", "18")],
            &[], &[],
        );
        assert_eq!(rows.len(), 2);
        // All-zero hours is what the writer renders as blank cells.
        assert_eq!(rows[1].units, hours("0", "0"));
    }

    #[test]
    fn salaried_keeps_hours_on_the_first_row() {
        let rows = build_block(
            true, Some(d("2000")),
            &[cat("teaching", "30", "0"), cat("office", "8", "0")],
            &[rate("teaching", "25")],
            &[], &[],
        );
        assert_eq!(labels(&rows), vec!["Salary: 2000.00"]);
        assert_eq!(rows[0].units, hours("38", "0"));
        assert!(!has_unassigned(&rows));
    }

    #[test]
    fn lump_sum_rate_gets_no_row() {
        // Lump-sum dollars ride in Other $$, so the rate must not become a row.
        let rows = build_block(
            false, None,
            &[cat("teaching", "10", "0")],
            &[rate("teaching", "25"), rate("recital", "50")],
            &[],
            &["recital".to_string()],
        );
        assert_eq!(labels(&rows), vec!["Hourly: 25.0000"]);
    }

    #[test]
    fn private_rows_carry_session_counts() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "10", "0")],
            &[rate("teaching", "25"), rate("private_30", "20"), rate("private_60", "35")],
            &[sessions(60, 2), sessions(30, 4)],
            &[],
        );
        assert_eq!(labels(&rows), vec![
            "Hourly: 25.0000",
            "Private Rate 1: 20.00 (per session)",
            "Private Rate 2: 35.00 (per session)",
        ]);
        assert_eq!(rows[1].units, RowUnits::Sessions(4));
        assert_eq!(rows[2].units, RowUnits::Sessions(2));
        // Sessions are counts, not hours: they must not land in the hour totals.
        assert_eq!(accounted(&rows), (d("10"), Decimal::ZERO, 6));
        assert!(!has_unassigned(&rows));
    }

    #[test]
    fn private_duration_with_no_sessions_gets_no_row() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "10", "0")],
            &[rate("teaching", "25"), rate("private_30", "20")],
            &[sessions(30, 0)],
            &[],
        );
        assert_eq!(labels(&rows), vec!["Hourly: 25.0000"]);
    }

    #[test]
    fn private_sessions_with_no_rate_are_kept() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "10", "0")],
            &[rate("teaching", "25")],
            &[sessions(60, 2)],
            &[],
        );
        assert_eq!(labels(&rows), vec!["Hourly: 25.0000", "private 60 min (no rate)"]);
        assert_eq!(rows[1].units, RowUnits::Sessions(2));
        assert!(!has_unassigned(&rows));
    }

    #[test]
    fn private_only_employee_starts_at_private_rate_1() {
        let rows = build_block(
            false, None, &[],
            &[rate("private_60", "35")],
            &[sessions(60, 3)],
            &[],
        );
        assert_eq!(labels(&rows), vec!["Private Rate 1: 35.00 (per session)"]);
    }

    #[test]
    fn private_numbering_is_separate_from_normal_rates() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "10", "0"), cat("office", "5", "0")],
            &[rate("teaching", "25"), rate("office", "18"),
              rate("private_30", "20"), rate("private_60", "35")],
            &[sessions(30, 3), sessions(60, 1)],
            &[],
        );
        assert_eq!(labels(&rows), vec![
            "Hourly: 25.0000",
            "Rate 2: 18.0000",
            "Private Rate 1: 20.00 (per session)",
            "Private Rate 2: 35.00 (per session)",
        ]);
    }

    #[test]
    fn label_matching_ignores_case_and_padding() {
        let rows = build_block(
            false, None,
            &[cat("Teaching", "12", "0")],
            &[rate(" teaching ", "25")],
            &[], &[],
        );
        assert_eq!(rows.len(), 1, "should match, not produce an orphan row");
        assert_eq!(rows[0].units, hours("12", "0"));
    }

    #[test]
    fn duplicate_rate_labels_do_not_double_count() {
        let rows = build_block(
            false, None,
            &[cat("teaching", "30", "0")],
            &[rate("teaching", "25"), rate("teaching", "27")],
            &[], &[],
        );
        assert_eq!(accounted(&rows), (d("30"), Decimal::ZERO, 0));
        assert_eq!(rows[1].units, hours("0", "0"));
    }

    #[test]
    fn everything_reconciles_on_a_mixed_block() {
        let categories = [cat("teaching", "30", "2"), cat("office", "8", "0"),
            cat("uncategorized", "1", "0")];
        let private_sessions = [sessions(30, 3), sessions(60, 1)];
        let rows = build_block(
            false, None,
            &categories,
            &[rate("teaching", "25"), rate("office", "18"), rate("private_30", "20")],
            &private_sessions,
            &[],
        );
        let expected_regular: Decimal = categories.iter().map(|c| c.regular_hours).sum();
        let expected_overtime: Decimal = categories.iter().map(|c| c.overtime_hours).sum();
        let expected_sessions: i32 = private_sessions.iter().map(|p| p.session_count).sum();
        assert_eq!(accounted(&rows), (expected_regular, expected_overtime, expected_sessions));
        assert!(!has_unassigned(&rows), "orphan rows should absorb everything");
        assert_eq!(labels(&rows), vec![
            "Hourly: 25.0000",
            "Rate 2: 18.0000",
            "Private Rate 1: 20.00 (per session)",
            "private 60 min (no rate)",
            "uncategorized (no rate)",
        ]);
    }

    #[test]
    fn employee_with_nothing_still_gets_one_row() {
        let rows = build_block(false, None, &[], &[], &[], &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].units, RowUnits::Empty);
    }
}
