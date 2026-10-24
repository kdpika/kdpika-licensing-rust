use std::{error::Error as StdError, fmt};

/// Safe SDK/server decision codes. Never contains response bodies or secrets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
    Configuration,
    Storage,
    Busy,
    NotActivated,
    AlreadyActivated,
    InvalidLicense,
    DeviceLimitReached,
    ActivationLimitReached,
    AppVersionNotSupported,
    DeviceNotAuthorized,
    LicensePending,
    InvalidRequest,
    PlanNotFound,
    LicenseExpired,
    LicenseRevoked,
    LicenseSuspended,
    LicenseCancelled,
    LicenseNotActive,
    LicenseNotStarted,
    InvalidToken,
    DeviceMismatch,
    RateLimited,
    InvalidResponse,
    SignatureInvalid,
    KeyRotationRequired,
    Network,
    InvalidSignedTime,
    ClockRollbackDetected,
    OfflineCacheInvalid,
    OfflineCacheMissing,
    OfflineGraceExpired,
    Http,
}
impl ErrorCode {
    /// Stable identifiers, matching API v1 where the code originates on the server.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configuration => "CONFIGURATION_ERROR",
            Self::Storage => "STORAGE_ERROR",
            Self::Busy => "STORAGE_BUSY",
            Self::NotActivated => "NOT_ACTIVATED",
            Self::AlreadyActivated => "ALREADY_ACTIVATED",
            Self::InvalidLicense => "INVALID_LICENSE",
            Self::DeviceLimitReached => "DEVICE_LIMIT_REACHED",
            Self::ActivationLimitReached => "ACTIVATION_LIMIT_REACHED",
            Self::AppVersionNotSupported => "APP_VERSION_NOT_SUPPORTED",
            Self::DeviceNotAuthorized => "DEVICE_NOT_AUTHORIZED",
            Self::LicensePending => "LICENSE_PENDING",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::PlanNotFound => "PLAN_NOT_FOUND",
            Self::LicenseExpired => "LICENSE_EXPIRED",
            Self::LicenseRevoked => "LICENSE_REVOKED",
            Self::LicenseSuspended => "LICENSE_SUSPENDED",
            Self::LicenseCancelled => "LICENSE_CANCELLED",
            Self::LicenseNotActive => "LICENSE_NOT_ACTIVE",
            Self::LicenseNotStarted => "LICENSE_NOT_STARTED",
            Self::InvalidToken => "INVALID_TOKEN",
            Self::DeviceMismatch => "DEVICE_MISMATCH",
            Self::RateLimited => "RATE_LIMITED",
            Self::InvalidResponse => "INVALID_RESPONSE",
            Self::SignatureInvalid => "SIGNATURE_INVALID",
            Self::KeyRotationRequired => "KEY_ROTATION_REQUIRED",
            Self::Network => "NETWORK_ERROR",
            Self::InvalidSignedTime => "INVALID_SIGNED_TIME",
            Self::ClockRollbackDetected => "CLOCK_ROLLBACK_DETECTED",
            Self::OfflineCacheInvalid => "OFFLINE_CACHE_INVALID",
            Self::OfflineCacheMissing => "OFFLINE_CACHE_MISSING",
            Self::OfflineGraceExpired => "OFFLINE_GRACE_EXPIRED",
            Self::Http => "HTTP_ERROR",
        }
    }
    pub(crate) fn from_server(code: &str) -> Self {
        match code {
            "INVALID_LICENSE" => Self::InvalidLicense,
            "DEVICE_LIMIT_REACHED" | "MAX_DEVICES_REACHED" => Self::DeviceLimitReached,
            "ACTIVATION_LIMIT_REACHED" => Self::ActivationLimitReached,
            "APP_VERSION_NOT_SUPPORTED" => Self::AppVersionNotSupported,
            "DEVICE_NOT_AUTHORIZED" => Self::DeviceNotAuthorized,
            "LICENSE_PENDING" => Self::LicensePending,
            "INVALID_REQUEST" => Self::InvalidRequest,
            "PLAN_NOT_FOUND" => Self::PlanNotFound,
            "LICENSE_EXPIRED" => Self::LicenseExpired,
            "LICENSE_REVOKED" => Self::LicenseRevoked,
            "LICENSE_SUSPENDED" => Self::LicenseSuspended,
            "LICENSE_CANCELLED" => Self::LicenseCancelled,
            "LICENSE_NOT_ACTIVE" => Self::LicenseNotActive,
            "INVALID_TOKEN" => Self::InvalidToken,
            "DEVICE_MISMATCH" => Self::DeviceMismatch,
            "RATE_LIMITED" => Self::RateLimited,
            _ => Self::Http,
        }
    }
    /// Plain, bounded, SDK-owned text suitable for displaying to a customer.
    pub fn message(self) -> &'static str {
        match self {
            Self::Configuration => "Check the application's licensing configuration.",
            Self::Storage | Self::Busy => {
                "Private license storage is unavailable or in use. Please try again."
            }
            Self::NotActivated => "Enter your license key to activate this installation.",
            Self::AlreadyActivated => {
                "This installation is already activated. Check its license or deactivate it first."
            }
            Self::InvalidLicense => "Check your license key, application and customer email.",
            Self::DeviceLimitReached | Self::ActivationLimitReached => {
                "Release an old installation or contact the software publisher."
            }
            Self::AppVersionNotSupported => "Update the application or contact its publisher.",
            Self::LicenseExpired => "Your license has expired. Contact the software publisher.",
            Self::LicenseSuspended => "This license is paused. Contact the software publisher.",
            Self::LicenseRevoked | Self::LicenseCancelled | Self::LicenseNotActive => {
                "This license is not active. Contact the software publisher."
            }
            Self::LicenseNotStarted | Self::LicensePending => {
                "This license is not ready yet. Contact the software publisher."
            }
            Self::InvalidToken | Self::DeviceMismatch | Self::DeviceNotAuthorized => {
                "Activate this installation again with your license key."
            }
            Self::Network => "The licensing service could not be reached. Check your connection.",
            Self::RateLimited => "Please wait before trying this license operation again.",
            Self::ClockRollbackDetected => "Check your computer's date and time, then reconnect.",
            Self::OfflineCacheMissing | Self::OfflineCacheInvalid | Self::OfflineGraceExpired => {
                "Connect to the internet to check your license."
            }
            Self::KeyRotationRequired => {
                "The signing key changed. Contact the application publisher for an update."
            }
            _ => {
                "Your license could not be confirmed. Reconnect or contact the software publisher."
            }
        }
    }
}
/// Initialization/storage failure. Display and Debug never include private paths or input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(pub(crate) ErrorCode);
impl Error {
    pub fn code(self) -> ErrorCode {
        self.0
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.0.as_str(), self.0.message())
    }
}
impl StdError for Error {}
pub(crate) type Result<T> = std::result::Result<T, Error>;
