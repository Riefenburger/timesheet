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
            let row = sqlx::query!(
                "SELECT employee_id FROM sessions WHERE token = $1 AND expires_at > now()",
                token
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