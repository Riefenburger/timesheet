use axum::{extract::State, http::StatusCode, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::CurrentEmployee;
use crate::auth_core::{generate_token, verify_password, SESSION_DAYS};
use crate::names::personal_name;
use crate::state::CookieConfig;
use crate::verify::{normalize_phone, VerifyError, VerifyService};
use crate::ratelimit;
use axum::http::HeaderMap;

use crate::auth::SuperAdminEmployee;
use crate::auth_core::hash_password;

const INVITE_DAYS: i64 = 7;


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
    // Confirm the employee exists and can still be given portal access.
    let employee = sqlx::query!(
        "SELECT id, is_active FROM employees WHERE id = $1", payload.employee_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match employee {
        None => return Err((StatusCode::NOT_FOUND, "Employee not found.".to_string())),
        Some(e) if !e.is_active => return Err((
            StatusCode::BAD_REQUEST,
            "This employee is deactivated. Reactivate them before sending an invite.".to_string(),
        )),
        Some(_) => {}
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
        SELECT e.name, e.first_name, e.last_name, e.middle_initial, e.email
        FROM invites i
        JOIN employees e ON e.id = i.employee_id
        WHERE i.token = $1 AND i.used_at IS NULL AND i.expires_at > now()
          AND e.is_active = true
        "#,
        token
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "This invite link is invalid or has expired.".to_string()))?;

    // "Welcome, Mark" — the person's own name, not the roster form.
    let employee_name = personal_name(
        row.first_name.as_deref(), row.middle_initial.as_deref(),
        row.last_name.as_deref(), row.name.as_deref(),
    );
    Ok(Json(InviteCheck { employee_name, email: row.email }))
}

#[derive(Deserialize)]
pub struct SignupInput {
    token: String,
    email: String,
    password: String,
    phone_number: String,
    code: String,
}

/// Create a session row and return its token. Shared by email login, signup and
/// phone login so the lifetime and the insert stay in one place.
async fn create_session(pool: &PgPool, employee_id: i64) -> Result<String, (StatusCode, String)> {
    let token = generate_token();
    let expires = Utc::now() + Duration::days(SESSION_DAYS);
    sqlx::query!(
        "INSERT INTO sessions (token, employee_id, expires_at) VALUES ($1, $2, $3)",
        token,
        employee_id,
        expires
    )
    .execute(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(token)
}

/// The session cookie. Secure in production (COOKIE_SECURE=true) so a
/// 120-day cookie is never sent in clear text; off for local HTTP dev.
fn session_cookie(token: String, cookies: CookieConfig) -> Cookie<'static> {
    Cookie::build(("session", token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(cookies.secure)
        .path("/")
        .max_age(time::Duration::days(SESSION_DAYS))
        .build()
}

// POST /auth/signup — consume an invite, set the password, log in. Public.
#[derive(Deserialize)]
pub struct SignupStartVerify {
    token: String,
    phone_number: String,
}

/// The invite predicate, identical in all three places that use it: unused,
/// unexpired, and belonging to an employee who is still active.
async fn employee_for_invite(
    pool: &PgPool,
    token: &str,
) -> Result<i64, (StatusCode, String)> {
    sqlx::query_scalar!(
        r#"
        SELECT i.employee_id
        FROM invites i
        JOIN employees e ON e.id = i.employee_id
        WHERE i.token = $1 AND i.used_at IS NULL AND i.expires_at > now()
          AND e.is_active = true
        "#,
        token
    )
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((
        StatusCode::BAD_REQUEST,
        "This invite link is invalid or has expired.".to_string(),
    ))
}

/// Normalise, and refuse a number another employee already holds.
///
/// Unlike the phone-LOGIN path, this does tell the caller the number is taken.
/// It is actionable feedback for someone setting up their own account, and the
/// surface is narrow: you need a valid unused invite to reach it at all.
async fn usable_phone(
    pool: &PgPool,
    raw: &str,
    employee_id: i64,
) -> Result<String, (StatusCode, String)> {
    let phone = normalize_phone(raw).ok_or((
        StatusCode::BAD_REQUEST,
        "That doesn't look like a valid phone number.".to_string(),
    ))?;

    let taken = sqlx::query_scalar!(
        "SELECT id FROM employees WHERE phone_number = $1 AND id != $2",
        phone,
        employee_id
    )
    .fetch_optional(pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if taken.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "That phone number is already linked to another account.".to_string(),
        ));
    }
    Ok(phone)
}

// POST /auth/signup/start-verify — text a code to the number being registered.
// PUBLIC, but gated by a valid unused invite token, so it is not an open SMS tap.
// (The open path is phone login; that gets the rate limiting in stage 3.)
pub async fn signup_start_verify(
    State(pool): State<PgPool>,
    State(verify): State<VerifyService>,
    Json(payload): Json<SignupStartVerify>,
) -> Result<StatusCode, (StatusCode, String)> {
    let employee_id = employee_for_invite(&pool, &payload.token).await?;
    let phone = usable_phone(&pool, &payload.phone_number, employee_id).await?;

    verify.start(&phone).await.map_err(|e| e.as_response())?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn signup(
    State(pool): State<PgPool>,
    State(cookies): State<CookieConfig>,
    jar: CookieJar,
    State(verify): State<VerifyService>,
    Json(payload): Json<SignupInput>,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    if payload.password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "Password must be at least 8 characters.".to_string()));
    }
    let email = payload.email.trim().to_lowercase();

    // Validate the invite and get the employee.
    let employee_id = employee_for_invite(&pool, &payload.token).await?;
    let phone = usable_phone(&pool, &payload.phone_number, employee_id).await?;

    // The code is checked HERE, at the finalising call, rather than trusting a
    // "verified" flag from the client — and Twilio will not approve the same
    // verification twice, so one check is the whole gate. A number only reaches
    // the column after this passes.
    let approved = verify
        .check(&phone, payload.code.trim())
        .await
        .map_err(|e| e.as_response())?;
    if !approved {
        return Err((
            StatusCode::UNAUTHORIZED,
            "That code didn't work or has expired. Request a new one.".to_string(),
        ));
    }

    let hash = hash_password(&payload.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Set the email + password on the employee, mark the invite used.
    // (Email may be updating from null or a placeholder to what they chose.)
    let mut tx = pool.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query!(
        "UPDATE employees SET email = $1, password_hash = $2, phone_number = $3 WHERE id = $4",
        email, hash, phone, employee_id
    )
    .execute(&mut *tx).await
    .map_err(|e| {
        // Unique violation on either email or phone_number. The uniqueness of
        // both was checked above, so this is the race-loser path.
        let msg = e.to_string();
        if msg.contains("phone_number") {
            (StatusCode::CONFLICT, "That phone number is already linked to another account.".to_string())
        } else {
            (StatusCode::CONFLICT, "That email is already in use.".to_string())
        }
    })?;

    sqlx::query!(
        "UPDATE invites SET used_at = now() WHERE token = $1", payload.token
    )
    .execute(&mut *tx).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Log them in immediately (create session + cookie).
    let token = create_session(&pool, employee_id).await?;
    Ok((jar.add(session_cookie(token, cookies)), StatusCode::NO_CONTENT))
}

#[derive(Deserialize)]
pub struct PhoneStart {
    phone_number: String,
}

#[derive(Deserialize)]
pub struct PhoneVerify {
    phone_number: String,
    code: String,
}

// POST /auth/phone/start — text a login code to a number.
//
// NON-DISCLOSURE: this returns 204 for ANY well-formed number, whether or not an
// account holds it. An unknown number simply gets no text. Returning an error
// would turn this endpoint into a "does this person work here?" oracle, and
// phone numbers are far more guessable than email addresses.
//
// The honest limit: someone who controls the number can tell "a text arrived"
// from "none did". That is inherent to SMS login and cannot be papered over —
// the rate limits, not the response shape, are what make enumeration expensive.
pub async fn phone_start(
    State(pool): State<PgPool>,
    State(verify): State<VerifyService>,
    headers: HeaderMap,
    Json(payload): Json<PhoneStart>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Malformed input is told so: that reveals nothing about who has an account.
    let phone = normalize_phone(&payload.phone_number).ok_or((
        StatusCode::BAD_REQUEST,
        "That doesn't look like a valid phone number.".to_string(),
    ))?;

    // A deployment with no Verify credentials is a global fact, independent of
    // the number, so saying so leaks nothing.
    if !verify.is_available() {
        return Err(VerifyError::NotConfigured.as_response());
    }

    let ip = ratelimit::client_ip(&headers);

    // Limits are applied BEFORE the lookup, and the attempt is recorded whatever
    // we decide, so the limiter behaves identically for known and unknown
    // numbers. Otherwise it would leak exactly what the 204 above conceals.
    ratelimit::check_start_allowed(&pool, &phone, ip.as_deref()).await?;
    ratelimit::record(&pool, &phone, ip.as_deref(), "start", false).await;

    let employee = sqlx::query_scalar!(
        "SELECT id FROM employees WHERE phone_number = $1 AND is_active = true",
        phone
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if employee.is_some() {
        // A per-request Twilio failure is logged and swallowed. Surfacing it
        // would mean only KNOWN numbers could produce a 503, which is the leak
        // again — the code screen offers resend and the password path instead.
        if let Err(e) = verify.start(&phone).await {
            eprintln!(
                "phone login: Verify start failed for {}: {:?}",
                crate::verify::masked(&phone),
                e
            );
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// POST /auth/phone/verify — exchange a texted code for a session.
pub async fn phone_verify(
    State(pool): State<PgPool>,
    State(verify): State<VerifyService>,
    State(cookies): State<CookieConfig>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<PhoneVerify>,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    let phone = normalize_phone(&payload.phone_number).ok_or((
        StatusCode::BAD_REQUEST,
        "That doesn't look like a valid phone number.".to_string(),
    ))?;
    let ip = ratelimit::client_ip(&headers);

    // Lockout is checked for unknown numbers too, so the lockout itself cannot
    // be used to tell known from unknown.
    ratelimit::check_code_allowed(&pool, &phone).await?;

    // ONE message for every cause: wrong code, expired code, already-used code,
    // unknown number, deactivated employee.
    let failed = || {
        (
            StatusCode::UNAUTHORIZED,
            "That code didn't work or has expired.".to_string(),
        )
    };

    let employee = sqlx::query_scalar!(
        "SELECT id FROM employees WHERE phone_number = $1 AND is_active = true",
        phone
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let Some(employee_id) = employee else {
        ratelimit::record(&pool, &phone, ip.as_deref(), "check", false).await;
        return Err(failed());
    };

    let approved = verify
        .check(&phone, payload.code.trim())
        .await
        .map_err(|e| e.as_response())?;

    ratelimit::record(&pool, &phone, ip.as_deref(), "check", approved).await;
    if !approved {
        return Err(failed());
    }

    let token = create_session(&pool, employee_id).await?;
    Ok((jar.add(session_cookie(token, cookies)), StatusCode::NO_CONTENT))
}

// POST /auth/login — verify email+password, create a session, set the cookie.
pub async fn login(
    State(pool): State<PgPool>,
    State(cookies): State<CookieConfig>,
    jar: CookieJar,
    Json(payload): Json<LoginInput>,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    let email = payload.email.trim().to_lowercase();

    // Look up the employee by email, with their hash.
    let row = sqlx::query!(
        "SELECT id, password_hash, is_active FROM employees WHERE lower(email) = $1",
        email
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Generic error whether the email is unknown or the password is wrong —
    // don't reveal which (avoids leaking who has an account).
    let (employee_id, hash, is_active) = match row {
        Some(r) => match r.password_hash {
            Some(h) => (r.id, h, r.is_active),
            None => return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string())),
        },
        None => return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string())),
    };

    if !verify_password(&payload.password, &hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password.".to_string()));
    }

    // Checked AFTER the password, on purpose. Telling an unauthenticated caller
    // that an address belongs to a deactivated account would leak who has one;
    // this way only the person who actually knows the password learns why they
    // are being turned away, and everyone else still gets the generic error.
    if !is_active {
        return Err((
            StatusCode::UNAUTHORIZED,
            "This account is no longer active — contact your administrator.".to_string(),
        ));
    }

    let token = create_session(&pool, employee_id).await?;
    Ok((jar.add(session_cookie(token, cookies)), StatusCode::NO_CONTENT))
}

// POST /auth/logout — delete the session and clear the cookie.
pub async fn logout(
    State(pool): State<PgPool>,
    State(cookies): State<CookieConfig>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), (StatusCode, String)> {
    if let Some(cookie) = jar.get("session") {
        let token = cookie.value().to_string();
        sqlx::query!("DELETE FROM sessions WHERE token = $1", token)
            .execute(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    // Removal has to match the original path/secure attributes, or the browser
    // may keep the cookie around.
    let mut gone = Cookie::from("session");
    gone.set_path("/");
    gone.set_secure(cookies.secure);
    Ok((jar.remove(gone), StatusCode::NO_CONTENT))
}

// GET /auth/me — who am I (if logged in).
pub async fn me(
    State(pool): State<PgPool>,
    State(cookies): State<CookieConfig>,
    jar: CookieJar,
    current: CurrentEmployee,
) -> Result<(CookieJar, Json<MeResponse>), (StatusCode, String)> {
    let row = sqlx::query!(
        "SELECT id, name, first_name, last_name, middle_initial, email, role FROM employees WHERE id = $1",
        current.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "Unknown employee".to_string()))?;

    // Shown in the nav and the My Time header, so personal format.
    let name = personal_name(
        row.first_name.as_deref(), row.middle_initial.as_deref(),
        row.last_name.as_deref(), row.name.as_deref(),
    );
    // CurrentEmployee has already refreshed the database expiry; re-issue the
    // cookie with a matching max-age so the browser's copy does not lapse out
    // from under an otherwise-active session. The SPA calls this on load, so an
    // active user's cookie is renewed on every visit.
    let refreshed = jar.get("session").map(|c| c.value().to_string());
    let jar = match refreshed {
        Some(token) => jar.add(session_cookie(token, cookies)),
        None => jar,
    };
    Ok((jar, Json(MeResponse { id: row.id, name, email: row.email, role: row.role })))
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