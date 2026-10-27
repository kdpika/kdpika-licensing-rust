//! Official KDPika Licensing client for native Rust applications and trusted backends.
//!
//! This client uses the existing API v1. It never creates licenses or processes
//! purchases. Never embed a developer management API key in a customer application.
//! See the shipped README for installation, offline policy and platform limits.
//!
//! ```no_run
//! use kdpika_licensing::{LicenseClient, Environment};
//! # fn main() -> Result<(), kdpika_licensing::Error> {
//! let client = LicenseClient::builder("YOUR_PUBLIC_APP_ID")
//!     .environment(Environment::Sandbox)
//!     .build()?;
//! let result = client.check();
//! if result.valid() && result.feature("pdf_export") {
//!     // Enable the feature only for this checked decision.
//! }
//! # Ok(()) }
//! ```
#[cfg(feature = "async")]
mod asynchronous;
mod client;
mod error;
mod fingerprint;
mod floating;
mod json;
mod policy;
mod result;
mod storage;
mod transport;
mod verification;
pub use fingerprint::HardwareFingerprint;
pub use policy::{CreditConsumption, CreditError};

#[cfg(feature = "async")]
pub use asynchronous::AsyncLicenseClient;
pub use client::{ClientBuilder, Environment, LicenseClient};
pub use error::{Error, ErrorCode};
pub use result::{LicenseResult, Source};
pub use transport::{HttpResponse, NetworkError, Request, Transport};
pub use verification::VerificationKey;
pub(crate) type Clock = std::sync::Arc<dyn Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync>;
/// SDK source version. Registry availability is a separate release step.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Signed application-family entitlement checks.
pub mod applications;
