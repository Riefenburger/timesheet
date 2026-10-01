//! Shared application state.
//!
//! The router's state is AppState, but every existing handler still takes
//! `State<PgPool>` and the auth extractors are still generic over
//! `S where PgPool: FromRef<S>`. That keeps working because `State<T>` resolves
//! through `FromRef`, so adding fields here costs no changes at the ~30 call
//! sites that only need the pool.

use axum::extract::FromRef;
use sqlx::PgPool;

use crate::verify::VerifyService;

/// Cookie attributes that differ between local dev and production.
#[derive(Clone, Copy)]
pub struct CookieConfig {
    /// Set the Secure flag, so the cookie is only ever sent over HTTPS.
    /// Off for local HTTP dev; COOKIE_SECURE=true in Railway.
    pub secure: bool,
}

impl CookieConfig {
    pub fn from_env() -> Self {
        CookieConfig {
            secure: std::env::var("COOKIE_SECURE").map(|v| v.trim() == "true").unwrap_or(false),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub verify: VerifyService,
    pub cookies: CookieConfig,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> PgPool {
        state.pool.clone()
    }
}

impl FromRef<AppState> for VerifyService {
    fn from_ref(state: &AppState) -> VerifyService {
        state.verify.clone()
    }
}

impl FromRef<AppState> for CookieConfig {
    fn from_ref(state: &AppState) -> CookieConfig {
        state.cookies
    }
}
