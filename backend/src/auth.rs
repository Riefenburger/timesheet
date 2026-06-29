use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};

// Who is making this request.
//
// STUB: for now we trust an `X-Employee-Id` header. When real Google auth
// arrives, ONLY the inside of from_request_parts changes — it'll validate a
// session token instead of trusting a header — and every handler that asks
// for a CurrentEmployee keeps working untouched.

pub struct CurrentEmployee {
    pub id: i64,
}

impl<S> FromRequestParts<S> for CurrentEmployee
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get("x-employee-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid X-Employee-Id header",
            ))?;

        Ok(CurrentEmployee { id })
    }
}