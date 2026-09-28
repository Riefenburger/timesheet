use axum::{extract::{State, Path}, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::auth::SuperAdminEmployee;

#[derive(Deserialize)]
pub struct NewEmployee {
    name: String,
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
    name: String,
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

#[derive(Deserialize)]
pub struct UpdateEmployee {
    name: String,
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
    // Friendly check: is the employee number already taken?
    if let Some(ref num) = payload.employee_number {
        let conflict = sqlx::query!(
            "SELECT name FROM employees WHERE employee_number = $1", num
        )
        .fetch_optional(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Some(row) = conflict {
            return Err((
                StatusCode::CONFLICT,
                format!("Employee number {} is already used by {}.", num, row.name),
            ));
        }
    }

    let employee = sqlx::query_as!(
        Employee,
        r#"
        INSERT INTO employees
            (name, employee_number, email, role, pay_method, pay_frequency, is_salaried, salary)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            id, name, employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        "#,
        payload.name,
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

    Ok((StatusCode::CREATED, Json(employee)))
}

pub async fn list_employees(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Employee>>, (StatusCode, String)> {
    let employees = sqlx::query_as!(
        Employee,
        r#"
        SELECT
            id, name, employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        FROM employees
        ORDER BY name
        "#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(employees))
}

// PUT /admin/employees/:id — update an employee. Super-admin only.
pub async fn update_employee(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(employee_id): Path<i64>,
    Json(payload): Json<UpdateEmployee>,
) -> Result<Json<Employee>, (StatusCode, String)> {
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
            "SELECT name FROM employees WHERE employee_number = $1 AND id != $2",
            num, employee_id
        )
        .fetch_optional(&pool).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Some(row) = conflict {
            return Err((
                StatusCode::CONFLICT,
                format!("Employee number {} is already used by {}.", num, row.name),
            ));
        }
    }

    // --- Do the update ---
    let employee = sqlx::query_as!(
        Employee,
        r#"
        UPDATE employees
        SET name = $1, employee_number = $2, email = $3,
            role = $4, pay_method = $5, pay_frequency = $6,
            is_salaried = $7, salary = $8
        WHERE id = $9
        RETURNING
            id, name, employee_number, email, google_sub,
            role, pay_method, pay_frequency, is_salaried, salary,
            (password_hash IS NOT NULL) AS "has_account!",
            is_active,
            created_at
        "#,
        payload.name,
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

    Ok(Json(employee))
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
        Employee,
        r#"
        UPDATE employees SET is_active = $1
        WHERE id = $2
        RETURNING
            id, name, employee_number, email, google_sub,
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

    Ok(Json(employee))
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
