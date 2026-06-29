use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use sqlx::PgPool;

// Who is making this request. STUB: trusts X-Employee-Id for now;
// real auth later changes only the inside of this, not any handler.
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

// Same identity, but ALSO proven to be an admin by checking the database.
// A handler that asks for this literally cannot run for a non-admin.
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
        // Reuse the identity logic — extractors can call other extractors.
        let current = CurrentEmployee::from_request_parts(parts, state)
            .await
            .map_err(|(code, msg)| (code, msg.to_string()))?;

        // Look up their REAL admin status in the database. Never trust the client for this.
        let pool = PgPool::from_ref(state);
        let is_admin = sqlx::query_scalar!(
            "SELECT is_admin FROM employees WHERE id = $1",
            current.id
        )
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::UNAUTHORIZED, "Unknown employee".to_string()))?;

        if !is_admin {
            return Err((StatusCode::FORBIDDEN, "Admin access required".to_string()));
        }

        Ok(AdminEmployee { id: current.id })
    }
}