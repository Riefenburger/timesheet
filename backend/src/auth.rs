use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_extra::extract::CookieJar;
use sqlx::PgPool;

// Who is making this request. Prefers a valid session cookie; falls back to the
// X-Employee-Id header (STUB) during the auth transition.
pub struct CurrentEmployee {
    pub id: i64,
}

impl<S> FromRequestParts<S> for CurrentEmployee
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Try the session cookie.
        let jar = CookieJar::from_headers(&parts.headers);
        if let Some(cookie) = jar.get("session") {
            let token = cookie.value();
            let pool = PgPool::from_ref(state);
            // Validate and refresh in ONE statement, so an authenticated request
            // still costs a single round trip.
            //
            // `valid` is the gate, unchanged in substance from before: the token
            // must exist, must not have expired, and the employee must still be
            // active (sessions are deleted on deactivation, so this is the
            // belt-and-braces check that a cookie cannot outlive it).
            //
            // `bumped` is the refresh-on-use. It is throttled to at most one
            // write per session per day: without the second condition every
            // authenticated request would become a write for no benefit. A
            // data-modifying CTE always executes even though nothing selects
            // from it, and it sees the statement's snapshot, so it cannot
            // interfere with `valid`.
            let row = sqlx::query!(
                r#"
                WITH valid AS (
                    SELECT s.token, s.employee_id
                    FROM sessions s
                    JOIN employees e ON e.id = s.employee_id
                    WHERE s.token = $1 AND s.expires_at > now() AND e.is_active = true
                ), bumped AS (
                    UPDATE sessions
                       SET expires_at = now() + make_interval(days => $2::int)
                     WHERE token = (SELECT token FROM valid)
                       AND expires_at < now() + make_interval(days => $2::int - 1)
                )
                SELECT employee_id AS "employee_id!" FROM valid
                "#,
                token,
                crate::auth_core::SESSION_DAYS as i32,
            )
            .fetch_optional(&pool)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Session lookup failed"))?;

            if let Some(r) = row {
                return Ok(CurrentEmployee { id: r.employee_id });
            }
            // Cookie present but invalid/expired — fall through to header for now.
        }

        // No valid session cookie → not authenticated. (Cookie-only; no header backdoor.)
        Err((StatusCode::UNAUTHORIZED, "Not logged in"))
    }
}

// Helper: fetch an employee's role. Shared by both gates.
async fn fetch_role<S>(parts: &mut Parts, state: &S) -> Result<(i64, String), (StatusCode, String)>
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    let current = CurrentEmployee::from_request_parts(parts, state)
        .await
        .map_err(|(code, msg)| (code, msg.to_string()))?;

    let pool = PgPool::from_ref(state);
    let role = sqlx::query_scalar!(
        "SELECT role FROM employees WHERE id = $1",
        current.id
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "Unknown employee".to_string()))?;

    Ok((current.id, role))
}

#[allow(dead_code)]
pub struct AdminEmployee {
    pub id: i64,
}

impl<S> FromRequestParts<S> for AdminEmployee
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let (id, role) = fetch_role(parts, state).await?;
        if role != "admin" && role != "super_admin" {
            return Err((StatusCode::FORBIDDEN, "Admin access required".to_string()));
        }
        Ok(AdminEmployee { id })
    }
}

#[allow(dead_code)]
pub struct SuperAdminEmployee {
    pub id: i64,
}

impl<S> FromRequestParts<S> for SuperAdminEmployee
where
    S: Send + Sync,
    PgPool: FromRef<S>,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let (id, role) = fetch_role(parts, state).await?;
        if role != "super_admin" {
            return Err((StatusCode::FORBIDDEN, "Super-admin access required".to_string()));
        }
        Ok(SuperAdminEmployee { id })
    }
}