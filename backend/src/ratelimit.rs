//! Rate limits for the phone-verification endpoints.
//!
//! Twilio Verify bills per verification attempt — materially more than a raw
//! SMS — so an unprotected start endpoint is a way to run up a bill, not just a
//! way to annoy someone. These limits are the cost control, and the global
//! ceiling is the backstop that holds even if the others are circumvented.
//!
//! Counters live in the database rather than in memory because Railway may run
//! more than one backend instance; an in-memory counter would be per-instance
//! and so would multiply the real ceiling by the instance count.

use axum::http::{HeaderMap, StatusCode};
use sqlx::PgPool;

/// Texts to one number. Three is enough for a bad signal or a mistyped number;
/// beyond that it is someone else's problem being inflicted on that handset.
const PER_PHONE_PER_15_MIN: i64 = 3;
const PER_PHONE_PER_DAY: i64 = 10;

/// Per source address. Best-effort only — see client_ip.
const PER_IP_PER_HOUR: i64 = 10;

/// The real cost ceiling: the most verifications this deployment will ever pay
/// for in an hour, whoever asks. ~35 staff, so legitimate traffic is nowhere
/// near this; tripping it means something is wrong, and the right response is to
/// stop spending money and let people use email & password.
const GLOBAL_PER_HOUR: i64 = 100;

/// Wrong codes for one number before that number is locked out for a while.
/// Twilio caps attempts per verification; this caps attempts across
/// verifications, so start/check cycling cannot brute-force a 6-digit code.
const CODE_FAILURES_BEFORE_LOCKOUT: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;

fn too_many() -> (StatusCode, String) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        "Too many attempts. Try again in a few minutes, or use email & password.".to_string(),
    )
}

fn ceiling_hit() -> (StatusCode, String) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "Phone login is temporarily unavailable — use email & password.".to_string(),
    )
}

/// The caller's address as best we can tell.
///
/// Behind Caddy and Railway's proxy the left-most X-Forwarded-For entry is the
/// claimed client address — and it is claimed, i.e. trivially spoofable by
/// sending the header yourself. We use it anyway because it does raise the cost
/// of casual enumeration, but it is NOT load-bearing: the per-number and global
/// limits are the ones that hold, and neither depends on this value.
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Decide whether to allow a "text me a code" request, given the counts in the
/// window. Pure, so the thresholds are testable without a database.
pub fn start_verdict(
    per_phone_15m: i64,
    per_phone_day: i64,
    per_ip_hour: i64,
    global_hour: i64,
) -> Result<(), (StatusCode, String)> {
    // Global first: once the ceiling is hit nothing else matters, and the
    // message should say the feature is off rather than blame the caller.
    if global_hour >= GLOBAL_PER_HOUR {
        return Err(ceiling_hit());
    }
    if per_phone_15m >= PER_PHONE_PER_15_MIN
        || per_phone_day >= PER_PHONE_PER_DAY
        || per_ip_hour >= PER_IP_PER_HOUR
    {
        return Err(too_many());
    }
    Ok(())
}

/// Count the windows and apply start_verdict. One query: the table is swept
/// daily, and the outer bound keeps the scan to a day of rows.
pub async fn check_start_allowed(
    pool: &PgPool,
    phone: &str,
    ip: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    let counts = sqlx::query!(
        r#"
        SELECT
            COUNT(*) FILTER (
                WHERE phone_number = $1 AND created_at > now() - interval '15 minutes'
            ) AS "per_phone_15m!",
            COUNT(*) FILTER (
                WHERE phone_number = $1
            ) AS "per_phone_day!",
            COUNT(*) FILTER (
                WHERE $2::text IS NOT NULL AND ip = $2
                  AND created_at > now() - interval '1 hour'
            ) AS "per_ip_hour!",
            COUNT(*) FILTER (
                WHERE created_at > now() - interval '1 hour'
            ) AS "global_hour!"
        FROM verification_attempts
        WHERE kind = 'start' AND created_at > now() - interval '1 day'
        "#,
        phone,
        ip,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    start_verdict(
        counts.per_phone_15m,
        counts.per_phone_day,
        counts.per_ip_hour,
        counts.global_hour,
    )
}

/// Is this number locked out for submitting too many wrong codes?
pub async fn check_code_allowed(
    pool: &PgPool,
    phone: &str,
) -> Result<(), (StatusCode, String)> {
    let failures = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "failures!"
        FROM verification_attempts
        WHERE kind = 'check' AND phone_number = $1 AND succeeded = false
          AND created_at > now() - make_interval(mins => $2::int)
        "#,
        phone,
        LOCKOUT_MINUTES as i32,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if failures >= CODE_FAILURES_BEFORE_LOCKOUT {
        return Err(too_many());
    }
    Ok(())
}

/// Log an attempt. Recorded for UNKNOWN numbers too: the limits must not behave
/// differently for a number that has an account, or the limiter itself becomes
/// the oracle the generic responses exist to avoid.
///
/// Best-effort — a logging failure must not take down a login, so it is reported
/// and swallowed rather than returned.
pub async fn record(pool: &PgPool, phone: &str, ip: Option<&str>, kind: &str, succeeded: bool) {
    if let Err(e) = sqlx::query!(
        r#"
        INSERT INTO verification_attempts (phone_number, ip, kind, succeeded)
        VALUES ($1, $2, $3, $4)
        "#,
        phone,
        ip,
        kind,
        succeeded,
    )
    .execute(pool)
    .await
    {
        eprintln!("rate limit: could not record a {kind} attempt: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_first_request_is_allowed() {
        assert!(start_verdict(0, 0, 0, 0).is_ok());
    }

    #[test]
    fn a_number_is_capped_per_quarter_hour() {
        assert!(start_verdict(PER_PHONE_PER_15_MIN - 1, 0, 0, 0).is_ok());
        let (code, _) = start_verdict(PER_PHONE_PER_15_MIN, 0, 0, 0).unwrap_err();
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn a_number_is_capped_per_day_even_when_spread_out() {
        // Under the 15-minute cap but over the daily one.
        let (code, _) = start_verdict(0, PER_PHONE_PER_DAY, 0, 0).unwrap_err();
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn an_address_is_capped_per_hour() {
        let (code, _) = start_verdict(0, 0, PER_IP_PER_HOUR, 0).unwrap_err();
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn the_global_ceiling_stops_spending_whoever_is_asking() {
        // A caller within every personal limit is still refused once the
        // deployment-wide hourly ceiling is reached.
        let (code, msg) = start_verdict(0, 0, 0, GLOBAL_PER_HOUR).unwrap_err();
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert!(msg.contains("email & password"), "must offer the fallback: {msg}");
    }

    #[test]
    fn the_ceiling_takes_precedence_over_the_personal_limits() {
        // Both tripped: the response should say the feature is unavailable
        // rather than blaming the caller.
        let (code, _) = start_verdict(99, 99, 99, GLOBAL_PER_HOUR).unwrap_err();
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn every_refusal_points_at_the_password_path() {
        for err in [too_many(), ceiling_hit()] {
            assert!(err.1.contains("email & password"), "{}", err.1);
        }
    }

    #[test]
    fn client_ip_reads_the_leftmost_forwarded_entry() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1, 10.0.0.2".parse().unwrap());
        assert_eq!(client_ip(&h).as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn client_ip_is_absent_when_unproxied_or_blank() {
        assert_eq!(client_ip(&HeaderMap::new()), None);
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "   ".parse().unwrap());
        assert_eq!(client_ip(&h), None);
    }
}
