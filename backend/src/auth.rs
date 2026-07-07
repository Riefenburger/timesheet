use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use sqlx::PgPool;

// Who is making this request. STUB: trusts X-Employee-Id for now.
pub struct CurrentEmployee {
    pub id: i64,
}

impl<S> FromRequestParts<S> for CurrentEmployee
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get("x-employee-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid X-Employee-Id header"))?;
        Ok(CurrentEmployee { id })
    }
}

// Helper: fetch an employee's role from the database. Shared by both gates.
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

// Proven to be an admin OR super_admin. Gates the hours/admin views.
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

// Proven to be a super_admin. Gates the pay/rate views.
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