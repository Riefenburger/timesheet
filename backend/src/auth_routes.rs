use axum::{extract::State, http::StatusCode, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::CurrentEmployee;
use crate::auth_core::{generate_token, verify_password};

use crate::auth::SuperAdminEmployee;
use crate::auth_core::hash_password;

const INVITE_DAYS: i64 = 7;

const SESSION_DAYS: i64 = 30;

#[derive(Deserialize)]
pub struct LoginInput {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    id: i64,
    name: String,
    email: Option<String>,
    role: String,
}

#[derive(Deserialize)]
pub struct InviteInput {
    employee_id: i64,
}

#[derive(Serialize)]
pub struct InviteResponse {
    token: String,
}

// POST /admin/invites — super-admin generates a signup invite for an employee.
pub async fn create_invite(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<InviteInput>,
) -> Result<Json<InviteResponse>, (StatusCode, String)> {
    // Confirm the employee exists.
    let exists = sqlx::query_scalar!(
        "SELECT id FROM employees WHERE id = $1", payload.employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Employee not found.".to_string()));
    }

    let token = generate_token();
    let expires = Utc::now() + Duration::days(INVITE_DAYS);
    sqlx::query!(
        "INSERT INTO invites (token, employee_id, expires_at) VALUES ($1, $2, $3)",
        token, payload.employee_id, expires
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(InviteResponse { token }))
}

#[derive(Serialize)]
pub struct InviteCheck {
    employee_name: String,
    email: Option<String>,
}

// GET /auth/invite/:token — validate an invite (for the signup page). Public.
pub async fn check_invite(
    State(pool): State<PgPool>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Json<InviteCheck>, (StatusCode, String)> {
    let row = sqlx::query!(
        r#"
        SELECT e.name, e.email
        FROM invites i
        JOIN employees e ON e.id = i.employee_id
        WHERE i.token = $1 AND i.used_at IS NULL AND i.expires_at > now()
        "#,
        token
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "This invite link is invalid or has expired.".to_string()))?;

    Ok(Json(InviteCheck { employee_name: row.name, email: row.email }))
}

#[derive(Deserialize)]
pub struct SignupInput {
    token: String,
    email: String,
    password: String,
}

// POST /auth/signup — consume an invite, set the password, log in. Public.
pub async fn signup(
    State(pool): State<PgPool>,
    jar: CookieJar,
    Json(payload): Json<SignupInput>,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    if payload.password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "Password must be at least 8 characters.".to_string()));
    }
    let email = payload.email.trim().to_lowercase();

    // Validate the invite and get the employee.
    let invite = sqlx::query!(
        r#"
        SELECT i.employee_id
        FROM invites i
        WHERE i.token = $1 AND i.used_at IS NULL AND i.expires_at > now()
        "#,
        payload.token
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::BAD_REQUEST, "This invite link is invalid or has expired.".to_string()))?;

    let employee_id = invite.employee_id;
    let hash = hash_password(&payload.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Set the email + password on the employee, mark the invite used.
    // (Email may be updating from null or a placeholder to what they chose.)
    let mut tx = pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query!(
        "UPDATE employees SET email = $1, password_hash = $2 WHERE id = $3",
        email, hash, employee_id
    )
    .execute(&mut *tx).await
    .map_err(|_e| {
        // Unique-violation on email → friendly message.
        (StatusCode::CONFLICT, "That email is already in use.".to_string())
    })?;

    sqlx::query!(
        "UPDATE invites SET used_at = now() WHERE token = $1", payload.token
    )
    .execute(&mut *tx).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Log them in immediately (create session + cookie).
    let token = generate_token();
    let expires = Utc::now() + Duration::days(SESSION_DAYS);
    sqlx::query!(
        "INSERT INTO sessions (token, employee_id, expires_at) VALUES ($1, $2, $3)",
        token, employee_id, expires
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let cookie = Cookie::build(("session", token))
        .http_only(true).same_site(SameSite::Lax).path("/")
        .max_age(time::Duration::days(SESSION_DAYS)).build();

    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

// POST /auth/login — verify email+password, create a session, set the cookie.
pub async fn login(
    State(pool): State<PgPool>,
    jar: CookieJar,
    Json(payload): Json<LoginInput>,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    let email = payload.email.trim().to_lowercase();

    // Look up the employee by email, with their hash.
    let row = sqlx::query!(
        "SELECT id, password_hash FROM employees WHERE lower(email) = $1",
        email
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Generic error whether the email is unknown or the password is wrong —
    // don't reveal which (avoids leaking who has an account).
    let (employee_id, hash) = match row {
        Some(r) => match r.password_hash {
            Some(h) => (r.id, h),
            None => return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string())),
        },
        None => return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string())),
    };

    if !verify_password(&payload.password, &hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string()));
    }

    // Create the session.
    let token = generate_token();
    let expires = Utc::now() + Duration::days(SESSION_DAYS);
    sqlx::query!(
        "INSERT INTO sessions (token, employee_id, expires_at) VALUES ($1, $2, $3)",
        token, employee_id, expires
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Set the httpOnly session cookie.
    let cookie = Cookie::build(("session", token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(SESSION_DAYS))
        .build();

    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

// POST /auth/logout — delete the session and clear the cookie.
pub async fn logout(
    State(pool): State<PgPool>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    if let Some(cookie) = jar.get("session") {
        let token = cookie.value().to_string();
        sqlx::query!("DELETE FROM sessions WHERE token = $1", token)
            .execute(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok((jar.remove(Cookie::from("session")), StatusCode::NO_CONTENT))
}

// GET /auth/me — who am I (if logged in).
pub async fn me(
    State(pool): State<PgPool>,
    current: CurrentEmployee,
) -> Result<Json<MeResponse>, (StatusCode, String)> {
    let row = sqlx::query!(
        "SELECT id, name, email, role FROM employees WHERE id = $1",
        current.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "Unknown employee".to_string()))?;

    Ok(Json(MeResponse { id: row.id, name: row.name, email: row.email, role: row.role }))
}

// POST /admin/employees/{id}/reset-account — super-admin clears an employee's
// password and kills their sessions, reverting them to "no account" so they can
// sign up fresh via a new invite.
pub async fn reset_account(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    axum::extract::Path(employee_id): axum::extract::Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Confirm the employee exists.
    let exists = sqlx::query_scalar!(
        "SELECT id FROM employees WHERE id = $1", employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Employee not found.".to_string()));
    }

    // Clear password + delete sessions atomically.
    let mut tx = pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query!(
        "UPDATE employees SET password_hash = NULL WHERE id = $1", employee_id
    )
    .execute(&mut *tx).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query!(
        "DELETE FROM sessions WHERE employee_id = $1", employee_id
    )
    .execute(&mut *tx).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}