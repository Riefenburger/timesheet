//! Phone verification via Twilio Verify.
//!
//! Twilio Verify owns the whole one-time-code lifecycle: it generates the code,
//! texts it, expires it and counts attempts. This module only ever calls two
//! endpoints — start a verification, and check a submitted code. We never
//! generate, store or compare codes ourselves, and no code is ever logged.
//!
//! There is no official Twilio Rust SDK, and the community crates cover
//! Programmable Messaging rather than Verify, so the two REST calls are made
//! directly with reqwest (Basic auth, form-encoded bodies).
//!
//! Substitutability is an enum rather than a trait object: the three modes are a
//! closed set, so matching on them avoids both `async_trait` and boxed futures
//! while giving tests and local dev the same "no network" guarantee.

use axum::http::StatusCode;
use std::sync::Arc;

/// The code DevVerify accepts. Local dev and tests only — see VerifyService::from_env.
pub const DEV_CODE: &str = "000000";

const VERIFY_BASE: &str = "https://verify.twilio.com/v2/Services";

#[derive(Debug, PartialEq)]
pub enum VerifyError {
    /// No Twilio credentials and no explicit dev mode. Phone login is simply off.
    NotConfigured,
    /// Twilio would not accept the request — e.g. an unusable number.
    Rejected,
    /// Twilio rate-limited us.
    RateLimited,
    /// Network failure, or a 5xx from Twilio.
    Unavailable(String),
}

impl VerifyError {
    /// The response to send. Every message names the email/password fallback,
    /// because that is the universal path when phone login cannot work.
    pub fn as_response(&self) -> (StatusCode, String) {
        match self {
            VerifyError::NotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Phone login isn't available right now — use email & password.".to_string(),
            ),
            VerifyError::Rejected => (
                StatusCode::BAD_REQUEST,
                "That phone number can't be used for verification.".to_string(),
            ),
            VerifyError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many attempts. Try again in a few minutes, or use email & password."
                    .to_string(),
            ),
            VerifyError::Unavailable(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Couldn't send a code right now — try again, or use email & password.".to_string(),
            ),
        }
    }
}

/// Keep enough of a number to correlate log lines, not enough to be a contact list.
pub fn masked(phone: &str) -> String {
    let n = phone.len();
    if n <= 6 {
        return "***".to_string();
    }
    format!("{}***{}", &phone[..2], &phone[n - 4..])
}

// Used by the signup and phone-login flows in stages 2 and 3.
#[allow(dead_code)]
/// Normalise user input to E.164, or None if it cannot be. The only writer of
/// employees.phone_number, so the column stays exactly-matchable for lookups.
///
/// Bare 10-digit and 1+10-digit input is assumed US/Canada, which is the only
/// region this studio employs in; anything else must be entered with a +prefix.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let explicit_plus = trimmed.starts_with('+');
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();

    if explicit_plus {
        // Already international: trust the country code, just validate the shape.
        if (8..=15).contains(&digits.len()) && !digits.starts_with('0') {
            return Some(format!("+{}", digits));
        }
        return None;
    }

    match digits.len() {
        10 => Some(format!("+1{}", digits)),
        11 if digits.starts_with('1') => Some(format!("+{}", digits)),
        _ => None,
    }
}

/// The live Twilio client. One per process: reqwest::Client holds its own
/// connection pool and is cheap to clone.
pub struct TwilioVerify {
    http: reqwest::Client,
    account_sid: String,
    auth_token: String,
    service_sid: String,
}

impl TwilioVerify {
    async fn post(&self, path: &str, form: &[(&str, &str)]) -> Result<reqwest::Response, VerifyError> {
        let url = format!("{}/{}/{}", VERIFY_BASE, self.service_sid, path);
        self.http
            .post(url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .form(form)
            .send()
            .await
            .map_err(|e| VerifyError::Unavailable(e.to_string()))
    }
}

#[derive(Clone)]
pub enum VerifyService {
    Live(Arc<TwilioVerify>),
    /// Local dev and tests: nothing is sent, DEV_CODE is the only valid code.
    Dev,
    /// No credentials and no explicit opt-in to dev mode. Phone endpoints 503.
    Unconfigured,
}

impl VerifyService {
    /// Resolve the mode from the environment — FAIL CLOSED.
    ///
    /// Dev mode is never inferred from missing credentials: if it were, one
    /// mistyped Railway variable would silently put production into a state
    /// where DEV_CODE logs in as anybody. Missing credentials means phone login
    /// is OFF (503), and dev mode requires an explicit SMS_DEV_MODE=true.
    ///
    /// Credentials win over SMS_DEV_MODE, so setting that variable somewhere it
    /// does not belong cannot downgrade a configured deployment either.
    pub fn from_env() -> Self {
        let var = |k: &str| {
            std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
        };
        let (sid, token, service, dev) = (
            var("TWILIO_ACCOUNT_SID"),
            var("TWILIO_AUTH_TOKEN"),
            var("TWILIO_VERIFY_SERVICE_SID"),
            var("SMS_DEV_MODE"),
        );

        match resolve_mode(sid.as_deref(), token.as_deref(), service.as_deref(), dev.as_deref()) {
            Mode::Live => VerifyService::Live(Arc::new(TwilioVerify {
                http: reqwest::Client::new(),
                account_sid: sid.expect("resolve_mode returns Live only when all three are set"),
                auth_token: token.expect("ditto"),
                service_sid: service.expect("ditto"),
            })),
            Mode::Dev => VerifyService::Dev,
            Mode::Unconfigured => VerifyService::Unconfigured,
        }
    }

    /// Logged at startup so a misconfigured deploy is obvious immediately.
    pub fn describe(&self) -> String {
        match self {
            VerifyService::Live(t) => {
                format!("live (Verify service {})", masked(&t.service_sid))
            }
            VerifyService::Dev => format!(
                "DEV MODE — no texts are sent, the only valid code is {}",
                DEV_CODE
            ),
            VerifyService::Unconfigured => {
                "not configured — phone login is disabled, email & password only".to_string()
            }
        }
    }

    pub fn is_available(&self) -> bool {
        !matches!(self, VerifyService::Unconfigured)
    }

    /// Ask Twilio to text a code. Callers must have already decided the number
    /// is worth texting (it exists, and is within the rate limits).
    pub async fn start(&self, phone: &str) -> Result<(), VerifyError> {
        match self {
            VerifyService::Unconfigured => Err(VerifyError::NotConfigured),
            VerifyService::Dev => {
                println!("[sms dev] would text a code to {}", masked(phone));
                Ok(())
            }
            VerifyService::Live(t) => {
                let res = t.post("Verifications", &[("To", phone), ("Channel", "sms")]).await?;
                classify_start(res.status().as_u16())
            }
        }
    }

    /// Validate a submitted code. Ok(false) means "wrong, expired, or already
    /// used" — the caller must not distinguish those for the user.
    pub async fn check(&self, phone: &str, code: &str) -> Result<bool, VerifyError> {
        match self {
            VerifyService::Unconfigured => Err(VerifyError::NotConfigured),
            VerifyService::Dev => Ok(code == DEV_CODE),
            VerifyService::Live(t) => {
                let res = t.post("VerificationCheck", &[("To", phone), ("Code", code)]).await?;
                let status = res.status().as_u16();
                // 404 = no such pending verification: expired, already approved,
                // or never started. Indistinguishable from a wrong code, by design.
                if status == 404 {
                    return Ok(false);
                }
                if let Err(e) = classify_start(status) {
                    return Err(e);
                }
                let body: CheckResponse = res
                    .json()
                    .await
                    .map_err(|e| VerifyError::Unavailable(e.to_string()))?;
                Ok(body.valid.unwrap_or(false) && body.status.as_deref() == Some("approved"))
            }
        }
    }
}

/// Which mode the configuration implies. Separated from from_env so the
/// fail-closed rules are testable without mutating process-wide environment
/// variables (and without tests racing each other over them).
#[derive(Debug, PartialEq)]
pub enum Mode {
    Live,
    Dev,
    Unconfigured,
}

/// Values are expected pre-trimmed, with blanks already mapped to None.
pub fn resolve_mode(
    account_sid: Option<&str>,
    auth_token: Option<&str>,
    service_sid: Option<&str>,
    dev_flag: Option<&str>,
) -> Mode {
    match (account_sid, auth_token, service_sid) {
        // Credentials win, so a stray SMS_DEV_MODE cannot downgrade a configured
        // deployment into one that accepts DEV_CODE.
        (Some(_), Some(_), Some(_)) => Mode::Live,
        // Dev mode ONLY on an explicit opt-in. Never inferred from missing
        // credentials: one mistyped Railway variable would otherwise put
        // production into a state where DEV_CODE logs in as anybody.
        _ if dev_flag == Some("true") => Mode::Dev,
        _ => Mode::Unconfigured,
    }
}

#[derive(serde::Deserialize)]
struct CheckResponse {
    status: Option<String>,
    valid: Option<bool>,
}

/// Map an HTTP status from Verify onto our error kinds.
fn classify_start(status: u16) -> Result<(), VerifyError> {
    match status {
        200..=299 => Ok(()),
        429 => Err(VerifyError::RateLimited),
        400..=499 => Err(VerifyError::Rejected),
        other => Err(VerifyError::Unavailable(format!("Twilio returned {}", other))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_common_us_input() {
        for raw in [
            "5551234567",
            "555-123-4567",
            "(555) 123-4567",
            " 555.123.4567 ",
            "15551234567",
            "1 (555) 123-4567",
            "+1 555 123 4567",
        ] {
            assert_eq!(normalize_phone(raw).as_deref(), Some("+15551234567"), "input: {raw:?}");
        }
    }

    #[test]
    fn keeps_explicit_international_numbers() {
        assert_eq!(normalize_phone("+44 20 7946 0958").as_deref(), Some("+442079460958"));
        assert_eq!(normalize_phone("+49-30-12345678").as_deref(), Some("+493012345678"));
    }

    #[test]
    fn rejects_input_that_is_not_a_phone_number() {
        for raw in ["", "   ", "555", "123456789", "abcdefghij", "+", "+0123456789"] {
            assert_eq!(normalize_phone(raw), None, "input: {raw:?}");
        }
    }

    #[test]
    fn rejects_numbers_that_are_too_long() {
        assert_eq!(normalize_phone("+1234567890123456"), None);
        assert_eq!(normalize_phone("123456789012"), None);
    }

    #[test]
    fn normalized_output_always_satisfies_the_db_check_constraint() {
        // Mirrors '^\+[1-9][0-9]{7,14}$' from the migration.
        for raw in ["5551234567", "+44 20 7946 0958", "1-555-123-4567"] {
            let out = normalize_phone(raw).unwrap();
            let digits = &out[1..];
            assert!(out.starts_with('+'));
            assert!(!digits.starts_with('0'));
            assert!((8..=15).contains(&digits.len()), "{out} has {} digits", digits.len());
            assert!(digits.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn masking_keeps_only_a_correlatable_fragment() {
        assert_eq!(masked("+15551234567"), "+1***4567");
        assert_eq!(masked("short"), "***");
    }

    // --- Fail-closed mode resolution ---

    #[test]
    fn missing_credentials_means_off_not_dev() {
        assert_eq!(resolve_mode(None, None, None, None), Mode::Unconfigured);
    }

    #[test]
    fn partial_credentials_are_neither_live_nor_dev() {
        assert_eq!(resolve_mode(Some("AC1"), None, None, None), Mode::Unconfigured);
        assert_eq!(resolve_mode(Some("AC1"), Some("tok"), None, None), Mode::Unconfigured);
        assert_eq!(resolve_mode(None, Some("tok"), Some("VA1"), None), Mode::Unconfigured);
    }

    #[test]
    fn dev_mode_requires_an_explicit_true() {
        assert_eq!(resolve_mode(None, None, None, Some("true")), Mode::Dev);
        // Anything else, including truthy-looking values, does not enable it.
        for flag in ["1", "yes", "TRUE", "True", "", "false"] {
            assert_eq!(resolve_mode(None, None, None, Some(flag)), Mode::Unconfigured, "flag: {flag:?}");
        }
    }

    #[test]
    fn credentials_beat_a_stray_dev_flag() {
        // The important one: SMS_DEV_MODE left set in a configured environment
        // must NOT make DEV_CODE a valid login.
        assert_eq!(
            resolve_mode(Some("AC1"), Some("tok"), Some("VA1"), Some("true")),
            Mode::Live
        );
    }

    #[test]
    fn full_credentials_are_live() {
        assert_eq!(resolve_mode(Some("AC1"), Some("tok"), Some("VA1"), None), Mode::Live);
    }

    #[tokio::test]
    async fn dev_mode_accepts_only_the_dev_code_and_sends_nothing() {
        let svc = VerifyService::Dev;
        assert!(svc.start("+15551234567").await.is_ok());
        assert_eq!(svc.check("+15551234567", DEV_CODE).await, Ok(true));
        assert_eq!(svc.check("+15551234567", "123456").await, Ok(false));
    }

    #[tokio::test]
    async fn unconfigured_mode_refuses_both_calls() {
        let svc = VerifyService::Unconfigured;
        assert_eq!(svc.start("+15551234567").await, Err(VerifyError::NotConfigured));
        assert_eq!(svc.check("+15551234567", DEV_CODE).await, Err(VerifyError::NotConfigured));
        let (code, _) = VerifyError::NotConfigured.as_response();
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn twilio_status_mapping() {
        assert_eq!(classify_start(201), Ok(()));
        assert_eq!(classify_start(429), Err(VerifyError::RateLimited));
        assert_eq!(classify_start(400), Err(VerifyError::Rejected));
        assert!(matches!(classify_start(503), Err(VerifyError::Unavailable(_))));
    }
}
