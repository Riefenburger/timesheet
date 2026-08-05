use axum::{extract::{Path, State}, http::StatusCode, Json};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::SuperAdminEmployee;

#[derive(Serialize)]
pub struct PrivateDuration {
    id: i64,
    duration_minutes: i32,
    #[serde(with = "rust_decimal::serde::float")]
    global_rate: Decimal,
}

// GET /private-durations — list all duration tiers. Open (any logged-in user),
// so entry forms and grids can populate their duration options.
pub async fn list_durations(
    State(pool): State<PgPool>,
    _current: crate::auth::CurrentEmployee,
) -> Result<Json<Vec<PrivateDuration>>, (StatusCode, String)> {
    let rows = sqlx::query_as!(
        PrivateDuration,
        r#"SELECT id, duration_minutes, global_rate FROM private_durations ORDER BY duration_minutes"#
    )
    .fetch_all(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct NewDuration {
    duration_minutes: i32,
    #[serde(with = "rust_decimal::serde::float")]
    global_rate: Decimal,
}

// POST /admin/private-durations — add a duration tier. Super-admin only.
pub async fn create_duration(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<NewDuration>,
) -> Result<StatusCode, (StatusCode, String)> {
    if payload.duration_minutes <= 0 {
        return Err((StatusCode::BAD_REQUEST, "Duration must be positive.".to_string()));
    }
    let result = sqlx::query!(
        r#"INSERT INTO private_durations (duration_minutes, global_rate) VALUES ($1, $2)
           ON CONFLICT (duration_minutes) DO NOTHING"#,
        payload.duration_minutes, payload.global_rate,
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::CONFLICT, "That duration already exists.".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateDuration {
    #[serde(with = "rust_decimal::serde::float")]
    global_rate: Decimal,
}

// PUT /admin/private-durations/:id — update a tier's global rate. Super-admin only.
pub async fn update_duration(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDuration>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"UPDATE private_durations SET global_rate = $1 WHERE id = $2"#,
        payload.global_rate, id,
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Duration not found.".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

// DELETE /admin/private-durations/:id — remove a tier. Super-admin only.
pub async fn delete_duration(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        r#"DELETE FROM private_durations WHERE id = $1"#, id
    )
    .execute(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Duration not found.".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}