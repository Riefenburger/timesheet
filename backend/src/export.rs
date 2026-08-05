use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::HashMap;
use rust_xlsxwriter::{Workbook, Format, FormatAlign, FormatBorder};

use crate::auth::SuperAdminEmployee;
use crate::totals::compute_totals;
use chrono::NaiveDate;

#[derive(Deserialize)]
pub struct ExportQuery {
    period_start: NaiveDate,
    period_end: NaiveDate,
    run_number: String,
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
    let empinfo_fmt = Format::new().set_border(FormatBorder::Thin)
        .set_align(FormatAlign::Top).set_text_wrap();
    let cell_fmt = Format::new().set_border(FormatBorder::Thin);
    let dec = |d: Decimal| d.to_f64().unwrap_or(0.0);
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
        ("Run Number:", q.run_number.clone()),
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
    // Each employee occupies a block of rows: one row per info line (name, Emp#,
    // Hourly, Rate 2, …). Data values sit on the block's first row; the rows below
    // get blank bordered cells so the block reads as one bordered unit.
    let mut row = header_row + 1;
    for emp in &employees {
        let (is_salaried, salary) = salary_map.get(&emp.employee_id).copied()
            .unwrap_or((false, None));

        // First cell = name + Emp# + first rate (Hourly, or Salary). Extra rates
        // become their own rows below.
        let mut first_cell = vec![
            emp.employee_name.clone(),
            format!("Emp#: {}", emp.employee_number),
        ];
        let mut extra_rate_lines: Vec<String> = Vec::new();

        if is_salaried {
            let s = salary.map(|v| format!("{:.2}", dec(v))).unwrap_or_else(|| "0.00".to_string());
            first_cell.push(format!("Salary: {}", s));
        } else {
            let normal: Vec<&crate::totals::RateEntry> = emp.rates.iter()
                .filter(|rt| !rt.label.starts_with("private_") && !lump_sum_cats.contains(&rt.label))
                .collect();
            let active_durs: Vec<i32> = emp.private_sessions.iter()
                .filter(|p| p.session_count > 0)
                .map(|p| p.session_duration)
                .collect();
            let privates: Vec<&crate::totals::RateEntry> = emp.rates.iter()
                .filter(|rt| {
                    if let Some(rest) = rt.label.strip_prefix("private_") {
                        if let Ok(d) = rest.parse::<i32>() {
                            return active_durs.contains(&d);
                        }
                    }
                    false
                })
                .collect();

            let mut idx = 0;
            for rt in normal.iter().chain(privates.iter()) {
                if idx == 0 {
                    // First rate joins the name cell as "Hourly".
                    first_cell.push(format!("Hourly: {:.4}", dec(rt.amount)));
                } else {
                    // Additional rates each get their own row.
                    extra_rate_lines.push(format!("Rate {}: {:.4}", idx + 1, dec(rt.amount)));
                }
                idx += 1;
            }
        }

        // Column totals for this employee.
        let regular: Decimal = emp.categories.iter().map(|c| c.regular_hours).sum::<Decimal>()
            + emp.private_regular_hours;
        let overtime: Decimal = emp.categories.iter().map(|c| c.overtime_hours).sum::<Decimal>();
        let sick: Decimal = emp.categories.iter().map(|c| c.sick_hours).sum::<Decimal>();
        let other = emp.other_earn + emp.lump_sum_earn; // combined (typed + lump)

        let block_rows = 1 + extra_rate_lines.len() as u32; // first cell + extra rate rows
        let first = row;
        let last = row + block_rows - 1;

        // First row: the combined name/Emp#/Hourly cell (wrapped).
        sheet.write_with_format(first, 0, first_cell.join("\n").as_str(), &empinfo_fmt).map_err(xerr)?;
        // Set the first row's height to fit its lines.
        sheet.set_row_height(first, 15.0 * first_cell.len() as f64).map_err(xerr)?;

        // Extra rate rows below.
        for (i, line) in extra_rate_lines.iter().enumerate() {
            sheet.write_with_format(first + 1 + i as u32, 0, line.as_str(), &cell_fmt).map_err(xerr)?;
        }

        // Data columns: values on the first row, blank bordered cells below.
        let vals = [dec(regular), dec(overtime), dec(other),
            dec(emp.competition_earn), dec(emp.coaching_earn), dec(sick)];
        for (c, v) in vals.iter().enumerate() {
            let col = (c + 1) as u16;
            sheet.write_with_format(first, col, *v, &num_fmt).map_err(xerr)?;
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