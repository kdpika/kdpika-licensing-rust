use crate::{result::timestamp, verification::hash};
use chrono::{DateTime, Utc};
use serde_json::Value;
pub(crate) const MAX: u64 = 9007199254740991;
pub(crate) fn credits(p: &Value) -> bool {
    p["metered"].is_boolean()
        && ["credit_balance", "total_allocated"]
            .iter()
            .all(|k| p[*k].as_u64().is_some_and(|v| v <= MAX))
}
pub(crate) fn lease(p: &Value, token: Option<&str>, now: DateTime<Utc>) -> bool {
    match p.get("floating") {
        Some(Value::Null) => true,
        Some(f) if f["max_concurrent_sessions"].as_u64().is_some_and(|n| n > 0) => token
            .is_some_and(|t| {
                f["session"]["token_hash"] == hash(t.as_bytes())
                    && timestamp(&f["session"]["expires_at"]).is_ok_and(|end| now < end)
            }),
        _ => false,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditConsumption {
    pub transaction_id: String,
    pub consumed: u64,
    pub balance: u64,
    pub replayed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreditError {
    InsufficientCredits { balance: u64 },
    Operation(crate::Error),
}
impl CreditError {
    pub fn code(self) -> crate::ErrorCode {
        match self {
            Self::InsufficientCredits { .. } => crate::ErrorCode::InsufficientCredits,
            Self::Operation(e) => e.code(),
        }
    }
}
impl From<crate::Error> for CreditError {
    fn from(e: crate::Error) -> Self {
        Self::Operation(e)
    }
}
impl std::fmt::Display for CreditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code().message())
    }
}
impl std::error::Error for CreditError {}
