use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::SuperAdminEmployee;

#[derive(Serialize)]
pub struct Category {
    id: i64,
    name: String,
    is_private: bool,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct NewCategory {
    name: String,
    #[serde(default)]
    is_private: bool,
}

#[derive(Deserialize)]
pub struct UpdateCategory {
    name: String,
    is_private: bool,
}

// GET /categories — the global category list. Open (entry forms and grids read it).
pub async fn list_categories(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Category>>, (StatusCode, String)> {
    let categories = sqlx::query_as!(
        Category,
        r#"SELECT id, name, is_private, created_at FROM categories ORDER BY name"#
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(categories))
}

// POST /admin/categories — add a category. Super-admin only.
pub async fn create_category(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Json(payload): Json<NewCategory>,
) -> Result<(StatusCode, Json<Category>), (StatusCode, String)> {
    let name = payload.name.trim().to_lowercase();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Category name can't be empty.".to_string()));
    }

    // Friendly duplicate check (name is UNIQUE in the table).
    let existing = sqlx::query!(
        "SELECT name FROM categories WHERE name = $1", name
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, format!("A category named \"{}\" already exists.", name)));
    }

    let category = sqlx::query_as!(
        Category,
        r#"
        INSERT INTO categories (name, is_private)
        VALUES ($1, $2)
        RETURNING id, name, is_private, created_at
        "#,
        name,
        payload.is_private,
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(category)))
}

// PUT /admin/categories/:id — rename or reflag a category. Super-admin only.
pub async fn update_category(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(category_id): Path<i64>,
    Json(payload): Json<UpdateCategory>,
) -> Result<Json<Category>, (StatusCode, String)> {
    let name = payload.name.trim().to_lowercase();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Category name can't be empty.".to_string()));
    }

    // Duplicate check excluding this category.
    let conflict = sqlx::query!(
        "SELECT name FROM categories WHERE name = $1 AND id != $2", name, category_id
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if conflict.is_some() {
        return Err((StatusCode::CONFLICT, format!("A category named \"{}\" already exists.", name)));
    }

    let category = sqlx::query_as!(
        Category,
        r#"
        UPDATE categories
        SET name = $1, is_private = $2
        WHERE id = $3
        RETURNING id, name, is_private, created_at
        "#,
        name,
        payload.is_private,
        category_id,
    )
    .fetch_optional(&pool).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Category not found".to_string()))?;

    Ok(Json(category))
}

// DELETE /admin/categories/:id — remove a category. Super-admin only.
pub async fn delete_category(
    State(pool): State<PgPool>,
    _super: SuperAdminEmployee,
    Path(category_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query!(
        "DELETE FROM categories WHERE id = $1", category_id
    )
    .execute(&pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Category not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}