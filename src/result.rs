use crate::{
    error::{Error, ErrorCode, Result},
    Clock,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::{fmt, time::Instant};

/// Where the checked decision came from. Acknowledgement is never an access grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Online,
    Cache,
    Offline,
    Local,
    Acknowledgement,
}
/// Immutable checked decision. Private fields prevent constructing a forged public grant.
pub struct LicenseResult {
    pub(crate) accepted: bool,
    pub(crate) signature: bool,
    pub(crate) source: Source,
    pub(crate) payload: Value,
    pub(crate) error: Option<ErrorCode>,
    pub(crate) request_id: Option<String>,
    pub(crate) retry_after: Option<String>,
    pub(crate) clock: Clock,
    pub(crate) checked_at: DateTime<Utc>,
    pub(crate) monotonic: Instant,
    pub(crate) session_guard: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}
impl fmt::Debug for LicenseResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LicenseResult")
            .field("valid", &self.valid())
            .field("signature_valid", &self.signature)
            .field("source", &self.source)
            .field("error_code", &self.error)
            .field("request_id", &self.request_id)
            .finish_non_exhaustive()
    }
}
impl LicenseResult {
    /// Is this a current access decision? Re-evaluates signed times at use.
    pub fn valid(&self) -> bool {
        self.allowed()
    }
    pub fn signature_valid(&self) -> bool {
        self.signature
    }
    pub fn source(&self) -> Source {
        self.source
    }
    pub fn error_code(&self) -> Option<ErrorCode> {
        self.error
    }
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
    /// Provider cooldown hint, never automatically retried by the SDK.
    pub fn retry_after(&self) -> Option<&str> {
        self.retry_after.as_deref()
    }
    /// Server-confirmed release; NOT a signed license/access grant.
    pub fn deactivated(&self) -> bool {
        self.accepted && self.source == Source::Acknowledgement
    }
    pub fn message(&self) -> &'static str {
        if let Some(e) = self.error {
            e.message()
        } else if self.deactivated() {
            "This installation has been deactivated."
        } else if self.valid() {
            "Your license is ready."
        } else {
            "Check your license again before continuing."
        }
    }
    pub fn status(&self) -> &str {
        self.payload["status"].as_str().unwrap_or("unknown")
    }
    pub fn license_id(&self) -> Option<&str> {
        self.payload["license_id"].as_str()
    }
    /// None means no expiry only for an authenticated, otherwise valid result.
    pub fn expires_at(&self) -> Option<&str> {
        self.payload["expires_at"].as_str()
    }
    pub fn offline_grace_until(&self) -> Option<&str> {
        self.payload["offline_grace_until"].as_str()
    }
    pub fn feature(&self, name: &str) -> bool {
        match self.feature_value(name) {
            Some(Value::Bool(v)) => v,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|v| v.is_finite() && v > 0.0),
            _ => false,
        }
    }
    /// Detached entitlement; never returns a value on invalid/unsigned/expired access.
    pub fn feature_value(&self, name: &str) -> Option<Value> {
        if !self.allowed() {
            return None;
        }
        self.payload["features"].get(name).cloned()
    }
    /// Numeric integer limit; negative, fractional, boolean or missing values are not unlimited.
    pub fn limit(&self, name: &str) -> Option<u64> {
        self.feature_value(name)?.as_u64()
    }
    fn allowed(&self) -> bool {
        if !self.accepted
            || !self.signature
            || self.source == Source::Acknowledgement
            || self.status() != "active"
        {
            return false;
        }
        let Ok(t) = Times::parse(&self.payload) else {
            return false;
        };
        let wall = (self.clock)();
        let elapsed = Duration::from_std(self.monotonic.elapsed()).unwrap_or(Duration::MAX);
        let now = wall.max(
            self.checked_at
                .checked_add_signed(elapsed)
                .unwrap_or(DateTime::<Utc>::MAX_UTC),
        );
        if !crate::policy::credits(&self.payload)
            || !crate::policy::lease(
                &self.payload,
                self.session_guard
                    .lock()
                    .ok()
                    .as_deref()
                    .and_then(|v| v.as_deref()),
                now,
            )
            || (self.payload["metered"] == true
                && (self.source != Source::Online || self.payload["credit_balance"] == 0))
        {
            return false;
        }
        if wall + Duration::seconds(300) < t.server
            || t.expiry.is_some_and(|v| now >= v)
            || t.starts.is_some_and(|v| now < v)
        {
            return false;
        }
        if matches!(self.source, Source::Cache | Source::Offline) {
            now <= t.grace
        } else {
            now < t.next
        }
    }
}
pub(crate) struct Times {
    pub server: DateTime<Utc>,
    pub next: DateTime<Utc>,
    pub grace: DateTime<Utc>,
    pub expiry: Option<DateTime<Utc>>,
    pub starts: Option<DateTime<Utc>>,
}
pub(crate) fn timestamp(value: &Value) -> Result<DateTime<Utc>> {
    let text = value.as_str().ok_or(Error(ErrorCode::InvalidSignedTime))?;
    if text.len() < 20 || text.len() > 40 || text.as_bytes().get(10).is_none_or(|b| *b != b'T') {
        return Err(Error(ErrorCode::InvalidSignedTime));
    }
    DateTime::parse_from_rfc3339(text)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| Error(ErrorCode::InvalidSignedTime))
}
impl Times {
    pub fn parse(p: &Value) -> Result<Self> {
        let server = timestamp(&p["server_time"])?;
        let next = timestamp(&p["next_validation_at"])?;
        let grace = timestamp(&p["offline_grace_until"])?;
        let expiry = match p.get("expires_at") {
            Some(Value::Null) => None,
            Some(v) => Some(timestamp(v)?),
            None => return Err(Error(ErrorCode::InvalidSignedTime)),
        };
        let starts = match p.get("starts_at") {
            None | Some(Value::Null) => None,
            Some(v) => Some(timestamp(v)?),
        };
        if next < server || grace < server {
            return Err(Error(ErrorCode::InvalidSignedTime));
        }
        Ok(Self {
            server,
            next,
            grace,
            expiry,
            starts,
        })
    }
}
