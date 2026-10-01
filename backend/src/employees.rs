use axum::{extract::{State, Path}, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::auth::SuperAdminEmployee;
use crate::names::roster_name;

#[derive(Deserialize)]
pub struct NewEmployee {
    first_name: String,
    last_name: String,
    #[serde(default)]
    middle_initial: Option<String>,
    employee_number: Option<String>,
    email: Option<String>,
    role: String,
    pay_method: String,
    pay_frequency: String,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
}

#[derive(Serialize)]
pub struct Employee {
    id: i64,
    // The pre-split single string. Kept until the back-fill is done, and shown in
    // the edit modal as "Currently stored as" so an admin can split it by hand.
    name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    middle_initial: Option<String>,
    // Roster format ("Last, MI First"), or the legacy string while unsplit.
    display_name: String,
    employee_number: Option<String>,
    email: Option<String>,
    google_sub: Option<String>,
    role: String,
    pay_method: String,
    pay_frequency: String,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
    has_account: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
}

// What the queries below select. Employee is built from it so display_name is
// formatted in exactly one place.
struct EmployeeRow {
    id: i64,
    name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    middle_initial: Option<String>,
    employee_number: Option<String>,
    email: Option<String>,
    google_sub: Option<String>,
    role: String,
    pay_method: String,
    pay_frequency: String,
    is_salaried: bool,
    salary: Option<Decimal>,
    has_account: bool,
    is_active: bool,
    created_at: DateTime<Utc>,
}

impl From<EmployeeRow> for Employee {
    fn from(r: EmployeeRow) -> Self {
        let display_name = roster_name(
            r.first_name.as_deref(),
            r.middle_initial.as_deref(),
            r.last_name.as_deref(),
            r.name.as_deref(),
        );
        Employee {
            id: r.id,
            name: r.name,
            first_name: r.first_name,
            last_name: r.last_name,
            middle_initial: r.middle_initial,
            display_name,
            employee_number: r.employee_number,
            email: r.email,
            google_sub: r.google_sub,
            role: r.role,
            pay_method: r.pay_method,
            pay_frequency: r.pay_frequency,
            is_salaried: r.is_salaried,
            salary: r.salary,
            has_account: r.has_account,
            is_active: r.is_active,
            created_at: r.created_at,
        }
    }
}

// This endpoint REPLACES every field, so serde's default treatment of a missing
// Option field — silently None — would null a column the caller never mentioned.
// On payroll data that is real data loss: forget `email` in a script and an
// employee quietly loses their login. Fail loud instead, naming what is missing.
fn require_fields(
    body: &serde_json::Value,
    fields: &[&str],
) -> Result<(), (StatusCode, String)> {
    let obj = body.as_object().ok_or((
        StatusCode::BAD_REQUEST,
        "Expected a JSON object.".to_string(),
    ))?;
    let missing: Vec<&str> = fields.iter().copied().filter(|f| !obj.contains_key(*f)).collect();
    if !missing.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Missing field(s): {}. This endpoint replaces every field, so each must be \
                 sent explicitly — use null to clear one.",
                missing.join(", ")
            ),
        ));
    }
    Ok(())
}

/// Every field UpdateEmployee replaces. Keep in step with that struct.
const UPDATE_EMPLOYEE_FIELDS: &[&str] = &[
    "first_name",
    "last_name",
    "middle_initial",
    "employee_number",
    "email",
    "role",
    "pay_method",
    "pay_frequency",
    "is_salaried",
    "salary",
];

// Trim to a value, or None when blank. Used for the optional middle initial.
fn trimmed(v: Option<&String>) -> Option<String> {
    v.map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_string())
}

// First/last are required; a middle initial is normalised to one uppercase letter.
fn name_parts(
    first: &str,
    last: &str,
    middle_initial: Option<&String>,
) -> Result<(String, String, Option<String>), (StatusCode, String)> {
    let first = first.trim().to_string();
    let last = last.trim().to_string();
    if first.is_empty() || last.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "First and last name are both required.".to_string()));
    }
    let mi = trimmed(middle_initial)
        .and_then(|s| s.chars().next())
        .map(|c| c.to_uppercase().to_string());
    Ok((first, last, mi))
}

#[derive(Deserialize)]
pub struct UpdateEmployee {
    first_name: String,
    last_name: String,
    middle_initial: Option<String>,
    employee_number: Option<String>,
    email: Option<String>,
    role: String,
    pay_method: String,
    pay_frequency: String,
    is_salaried: bool,
    #[serde(with = "rust_decimal::serde::float_option")]
    salary: Option<Decimal>,
}

pub async fn create_employee(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<NewEmployee>,
) -> Result<(StatusCode, Json<Employee>), (StatusCode, String)> {
    let (first, last, middle_initial) =
        name_parts(&payload.first_name, &payload.last_name, payload.middle_initial.as_ref())?;

    // Friendly check: is the employee number already taken?
    if let Some(ref num) = payload.employee_number {
        let conflict = sqlx::query!(
            "SELECT name, first_name, last_name, middle_initial FROM employees WHERE employee_number = $1",
            num
        )
        .fetch_optional(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Some(row) = conflict {
            return Err((
                StatusCode::CONFLICT,
                format!("Employee number {} is already used by {}.", num, roster_name(
                    row.first_name.as_deref(), row.middle_initial.as_deref(),
                    row.last_name.as_deref(), row.name.as_deref(),
                )),
            ));
        }
    }

    let employee = sqlx::query_as!(
        EmployeeRow,
        r#"
        INSERT INTO employees
            (first_name, last_name, middle_initial, employee_number, email,
             role, pay_method, pay_frequency, is_salaried, salary)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING
            id, name, first_name, last_name, middle_initial,
            employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        "#,
        first,
        last,
        middle_initial,
        payload.employee_number,
        payload.email,
        payload.role,
        payload.pay_method,
        payload.pay_frequency,
        payload.is_salaried,
        payload.salary,
    )
    .fetch_one(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(employee.into())))
}

pub async fn list_employees(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Employee>>, (StatusCode, String)> {
    let employees = sqlx::query_as!(
        EmployeeRow,
        r#"
        SELECT
            id, name, first_name, last_name, middle_initial,
            employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        FROM employees
        ORDER BY COALESCE(last_name, name), first_name
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employees.into_iter().map(Employee::from).collect()))
}

// PUT /admin/employees/:id — update an employee. Super-admin only.
pub async fn update_employee(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
    Json(raw): Json<serde_json::Value>,
) -> Result<Json<Employee>, (StatusCode, String)> {
    require_fields(&raw, UPDATE_EMPLOYEE_FIELDS)?;
    let payload: UpdateEmployee = serde_json::from_value(raw)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid field: {}", e)))?;

    let (first, last, middle_initial) =
        name_parts(&payload.first_name, &payload.last_name, payload.middle_initial.as_ref())?;

    // --- Guard: don't let the last super_admin lose that role ---
    // If this employee is currently super_admin and the new role isn't,
    // make sure at least one OTHER super_admin remains.
    let current_role = sqlx::query_scalar!(
        "SELECT role FROM employees WHERE id = $1",
        employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Employee not found".to_string()))?;

    if current_role == "super_admin" && payload.role != "super_admin" {
        let other_supers = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM employees WHERE role = 'super_admin' AND id != $1"#,
            employee_id
        )
        .fetch_one(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if other_supers == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "Cannot remove the last super-admin. Promote someone else first.".to_string(),
            ));
        }
    }

    // --- Friendly check: is the employee number taken by someone else? ---
    if let Some(ref num) = payload.employee_number {
        let conflict = sqlx::query!(
            r#"
            SELECT name, first_name, last_name, middle_initial
            FROM employees WHERE employee_number = $1 AND id != $2
            "#,
            num, employee_id
        )
        .fetch_optional(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Some(row) = conflict {
            return Err((
                StatusCode::CONFLICT,
                format!("Employee number {} is already used by {}.", num, roster_name(
                    row.first_name.as_deref(), row.middle_initial.as_deref(),
                    row.last_name.as_deref(), row.name.as_deref(),
                )),
            ));
        }
    }

    // --- Do the update ---
    let employee = sqlx::query_as!(
        EmployeeRow,
        r#"
        UPDATE employees
        SET first_name = $1, last_name = $2, middle_initial = $3,
            employee_number = $4, email = $5,
            role = $6, pay_method = $7, pay_frequency = $8,
            is_salaried = $9, salary = $10
        WHERE id = $11
        RETURNING
            id, name, first_name, last_name, middle_initial,
            employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        "#,
        first,
        last,
        middle_initial,
        payload.employee_number,
        payload.email,
        payload.role,
        payload.pay_method,
        payload.pay_frequency,
        payload.is_salaried,
        payload.salary,
        employee_id,
    )
    .fetch_one(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employee.into()))
}
// --- Deactivate / reactivate / hard-delete -------------------------------
//
// Deactivation is the preferred way to retire an employee: it keeps every row
// they own (entries, rates, typed totals) for payroll records, drops them from
// active rosters and revokes portal access. Hard-delete exists only for
// employees created in error and is refused the moment they have any history.

#[derive(Deserialize)]
pub struct SetActive {
    is_active: bool,
}

// Both guards below apply to deactivation AND deletion, so neither can lock
// everyone out of administration.
async fn guard_not_self_or_last_super_admin(
    pool: &PgPool,
    acting_id: i64,
    target_id: i64,
    verb: &str,
) -> Result<(), (StatusCode, String)> {
    if acting_id == target_id {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("You cannot {} your own account.", verb),
        ));
    }

    let target_role = sqlx::query_scalar!(
        "SELECT role FROM employees WHERE id = $1",
        target_id
    )
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Employee not found".to_string()))?;

    if target_role == "super_admin" {
        // Only *active* super-admins count as remaining cover: a deactivated one
        // can no longer sign in.
        let other_supers = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) AS "count!" FROM employees
            WHERE role = 'super_admin' AND is_active = true AND id != $1
            "#,
            target_id
        )
        .fetch_one(pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if other_supers == 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Cannot {} the last super-admin. Promote someone else first.", verb),
            ));
        }
    }

    Ok(())
}

// PUT /admin/employees/:id/active — deactivate or reactivate. Super-admin only.
// Deliberately separate from update_employee so a stale edit form can never
// silently flip someone's active state.
pub async fn set_employee_active(
    State(pool): State<PgPool>,
    super_admin: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
    Json(payload): Json<SetActive>,
) -> Result<Json<Employee>, (StatusCode, String)> {
    if !payload.is_active {
        guard_not_self_or_last_super_admin(&pool, super_admin.id, employee_id, "deactivate").await?;
    }

    let employee = sqlx::query_as!(
        EmployeeRow,
        r#"
        UPDATE employees SET is_active = $1
        WHERE id = $2
        RETURNING
            id, name, first_name, last_name, middle_initial,
            employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        "#,
        payload.is_active,
        employee_id,
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Employee not found".to_string()))?;

    // Deactivating revokes portal access, so drop their live sessions too —
    // otherwise an already-signed-in employee would keep working until their
    // cookie expired. (The login lookup and CurrentEmployee both require
    // is_active, so they cannot get back in.)
    if !payload.is_active {
        sqlx::query!("DELETE FROM sessions WHERE employee_id = $1", employee_id)
            .execute(&pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(employee.into()))
}

// DELETE /admin/employees/:id — permanent hard-delete. Super-admin only.
//
// Payroll history must never be deletable through this: time_entries,
// category_hours and period_dollar_totals all have ON DELETE RESTRICT, and none
// of those FKs is changed. The counts below exist to turn that database refusal
// into a message an admin can act on; the RESTRICT stays as the real backstop.
// employee_rates, invites, sessions and profiles cascade away on their own.
pub async fn delete_employee(
    State(pool): State<PgPool>,
    super_admin: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    guard_not_self_or_last_super_admin(&pool, super_admin.id, employee_id, "delete").await?;

    let history = sqlx::query!(
        r#"
        SELECT
            (SELECT COUNT(*) FROM time_entries         WHERE employee_id = $1) AS "entries!",
            (SELECT COUNT(*) FROM category_hours       WHERE employee_id = $1) AS "typed_totals!",
            (SELECT COUNT(*) FROM period_dollar_totals WHERE employee_id = $1) AS "dollar_totals!"
        "#,
        employee_id
    )
    .fetch_one(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if history.entries > 0 || history.typed_totals > 0 || history.dollar_totals > 0 {
        let mut parts: Vec<String> = Vec::new();
        if history.entries > 0 {
            parts.push(format!("{} time {}", history.entries,
                if history.entries == 1 { "entry" } else { "entries" }));
        }
        if history.typed_totals > 0 {
            parts.push(format!("{} typed total rows", history.typed_totals));
        }
        if history.dollar_totals > 0 {
            parts.push(format!("{} dollar-total rows", history.dollar_totals));
        }
        return Err((
            StatusCode::CONFLICT,
            format!(
                "This employee has {} and cannot be deleted — payroll records are kept. Deactivate them instead.",
                parts.join(", ")
            ),
        ));
    }

    let result = sqlx::query!("DELETE FROM employees WHERE id = $1", employee_id)
        .execute(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Employee not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full_body() -> serde_json::Value {
        json!({
            "first_name": "Mark", "last_name": "Doherty", "middle_initial": "J",
            "employee_number": "937", "email": "mark@example.com",
            "role": "super_admin", "pay_method": "payroll",
            "pay_frequency": "bimonthly", "is_salaried": false, "salary": null
        })
    }

    #[test]
    fn a_complete_body_passes() {
        assert!(require_fields(&full_body(), UPDATE_EMPLOYEE_FIELDS).is_ok());
    }

    #[test]
    fn an_explicit_null_is_accepted_as_clearing_the_field() {
        // Present-but-null is how a caller says "clear this".
        let mut body = full_body();
        body["email"] = serde_json::Value::Null;
        assert!(require_fields(&body, UPDATE_EMPLOYEE_FIELDS).is_ok());
    }

    #[test]
    fn a_missing_field_is_rejected_rather_than_silently_nulled() {
        // The regression this exists for: an omitted email used to wipe the
        // employee's login instead of failing.
        let mut body = full_body();
        body.as_object_mut().unwrap().remove("email");
        let (code, msg) = require_fields(&body, UPDATE_EMPLOYEE_FIELDS).unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
        assert!(msg.contains("email"), "message should name the field: {msg}");
    }

    #[test]
    fn every_missing_field_is_named_at_once() {
        let mut body = full_body();
        let obj = body.as_object_mut().unwrap();
        obj.remove("email");
        obj.remove("employee_number");
        obj.remove("salary");
        let (_, msg) = require_fields(&body, UPDATE_EMPLOYEE_FIELDS).unwrap_err();
        for f in ["email", "employee_number", "salary"] {
            assert!(msg.contains(f), "{f} missing from message: {msg}");
        }
    }

    #[test]
    fn an_empty_body_names_everything() {
        let (_, msg) = require_fields(&json!({}), UPDATE_EMPLOYEE_FIELDS).unwrap_err();
        for f in UPDATE_EMPLOYEE_FIELDS {
            assert!(msg.contains(f), "{f} missing from message: {msg}");
        }
    }

    #[test]
    fn a_non_object_body_is_rejected() {
        assert!(require_fields(&json!([1, 2, 3]), UPDATE_EMPLOYEE_FIELDS).is_err());
        assert!(require_fields(&json!("nope"), UPDATE_EMPLOYEE_FIELDS).is_err());
    }

    #[test]
    fn the_field_list_matches_what_the_struct_replaces() {
        // Guards against adding a column to UpdateEmployee and forgetting the
        // list, which would reopen the silent-null hole for that field.
        let body = full_body();
        let keys: Vec<&str> = body.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys.len(), UPDATE_EMPLOYEE_FIELDS.len());
        for k in keys {
            assert!(UPDATE_EMPLOYEE_FIELDS.contains(&k), "{k} not in UPDATE_EMPLOYEE_FIELDS");
        }
    }
}
