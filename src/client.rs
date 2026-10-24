use crate::{
    error::{Error, ErrorCode, Result},
    json,
    result::{timestamp, LicenseResult, Source, Times},
    storage::{self, State, Store},
    transport::{self, HttpTransport},
    verification::{self, VerificationKey},
    Clock, CreditConsumption, CreditError, HardwareFingerprint, HttpResponse, Request, Transport,
};
use chrono::{Duration as TimeDelta, Utc};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fmt,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

/// Explicit API environment. Sandbox and Live have separate identity/storage namespaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Sandbox,
    Live,
}
impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sandbox => "SANDBOX",
            Self::Live => "LIVE",
        }
    }
}
/// Builder contains trusted application configuration, not customer credentials.
pub struct ClientBuilder {
    product: String,
    environment: Environment,
    base_url: String,
    application_name: String,
    application_version: String,
    storage_dir: Option<PathBuf>,
    timeout: Duration,
    pin: Option<VerificationKey>,
    transport: Option<Arc<dyn Transport>>,
    clock: Clock,
    allow_loopback: bool,
    fingerprint: Arc<dyn Fn(&str) -> Result<HardwareFingerprint> + Send + Sync>,
}
impl ClientBuilder {
    /// Trusted native collection adapter, also usable by isolated synthetic tests.
    pub fn fingerprint_provider(
        mut self,
        value: impl Fn(&str) -> Result<HardwareFingerprint> + Send + Sync + 'static,
    ) -> Self {
        self.fingerprint = Arc::new(value);
        self
    }
    pub fn environment(mut self, value: Environment) -> Self {
        self.environment = value;
        self
    }
    /// HTTPS site origin only (not `/api/v1`, query strings or URL credentials).
    pub fn base_url(mut self, value: impl Into<String>) -> Self {
        self.base_url = value.into();
        self
    }
    pub fn application_name(mut self, value: impl Into<String>) -> Self {
        self.application_name = value.into();
        self
    }
    pub fn application_version(mut self, value: impl Into<String>) -> Self {
        self.application_version = value.into();
        self
    }
    /// Persistent private directory. The SDK adds a per-origin/product/environment namespace.
    pub fn storage_dir(mut self, value: impl Into<PathBuf>) -> Self {
        self.storage_dir = Some(value.into());
        self
    }
    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = value;
        self
    }
    /// Pin an independently trusted key. A changed key requires a publisher-controlled update.
    pub fn verification_key(mut self, value: VerificationKey) -> Self {
        self.pin = Some(value);
        self
    }
    /// Advanced, trusted application adapter; it must not reclassify HTTP refusals as network loss.
    pub fn transport(mut self, value: Arc<dyn Transport>) -> Self {
        self.transport = Some(value);
        self
    }
    /// Test-only clock injection. Production applications should keep the real UTC clock.
    pub fn clock(
        mut self,
        value: impl Fn() -> chrono::DateTime<Utc> + Send + Sync + 'static,
    ) -> Self {
        self.clock = Arc::new(value);
        self
    }
    /// Explicit local development only. Does not allow HTTP to non-loopback hosts.
    pub fn allow_loopback_http(mut self, value: bool) -> Self {
        self.allow_loopback = value;
        self
    }
    pub fn build(self) -> Result<LicenseClient> {
        if !safe_text(&self.product, 160)
            || !self.product.starts_with("app_")
            || !self
                .product
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || !safe_text(&self.application_name, 120)
            || self.application_version.len() > 64
            || self.application_version.chars().any(char::is_control)
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(120)
        {
            return Err(Error(ErrorCode::Configuration));
        }
        let origin = transport::safe_origin(&self.base_url, self.allow_loopback)?;
        if let Some(key) = &self.pin {
            key.validate()
                .map_err(|_| Error(ErrorCode::Configuration))?;
        }
        let root = match self.storage_dir {
            Some(p) => p,
            None => storage::default_root()?,
        };
        let namespace = format!(
            "rust-sdk-v1|{}|{}|{}",
            origin,
            self.product,
            self.environment.as_str()
        );
        let store = Store::new(&root, &namespace)?;
        let transport = match self.transport {
            Some(t) => t,
            None => Arc::new(HttpTransport::new(&origin, self.timeout)?) as Arc<dyn Transport>,
        };
        Ok(LicenseClient {
            inner: Arc::new_cyclic(|weak| ClientCore {
                product: self.product,
                environment: self.environment,
                application_name: self.application_name,
                application_version: self.application_version,
                store,
                pin: self.pin,
                transport,
                clock: self.clock,
                last_key_fetch: Mutex::new(None),
                fingerprint: self.fingerprint,
                session_guard: Arc::new(Mutex::new(None)),
                heartbeat: Arc::new(crate::floating::Heartbeat::new()),
                weak: weak.clone(),
            }),
        })
    }
}
/// One installation. All operations use an OS-held lock shared by processes using its namespace.
pub struct LicenseClient {
    inner: Arc<ClientCore>,
}
impl std::ops::Deref for LicenseClient {
    type Target = ClientCore;
    fn deref(&self) -> &ClientCore {
        &self.inner
    }
}
#[doc(hidden)]
pub struct ClientCore {
    product: String,
    environment: Environment,
    application_name: String,
    application_version: String,
    store: Store,
    pin: Option<VerificationKey>,
    transport: Arc<dyn Transport>,
    clock: Clock,
    last_key_fetch: Mutex<Option<Instant>>,
    fingerprint: Arc<dyn Fn(&str) -> Result<HardwareFingerprint> + Send + Sync>,
    session_guard: Arc<Mutex<Option<String>>>,
    heartbeat: Arc<crate::floating::Heartbeat>,
    weak: Weak<ClientCore>,
}
impl fmt::Debug for ClientCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LicenseClient")
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}
struct Reply {
    status: u16,
    value: Value,
    raw: Vec<u8>,
    id: Option<String>,
    retry: Option<String>,
}
impl Reply {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status) && self.value["success"] == true
    }
    fn code(&self) -> ErrorCode {
        if self.value["error"]["code"] == "INVALID_RESPONSE" {
            ErrorCode::InvalidResponse
        } else {
            ErrorCode::from_server(self.value["error"]["code"].as_str().unwrap_or(""))
        }
    }
}
impl LicenseClient {
    /// Defaults to Sandbox for a new Rust integration. Choose Live explicitly for customer sales.
    pub fn builder(product: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            product: product.into(),
            environment: Environment::Sandbox,
            base_url: "https://licensing.kdpika.com".into(),
            application_name: "Licensed Rust Application".into(),
            application_version: String::new(),
            storage_dir: None,
            timeout: Duration::from_secs(15),
            pin: None,
            transport: None,
            clock: Arc::new(Utc::now),
            allow_loopback: false,
            fingerprint: Arc::new(crate::fingerprint::collect),
        }
    }
}
impl fmt::Debug for LicenseClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
impl Drop for LicenseClient {
    fn drop(&mut self) {
        self.inner.heartbeat.stop();
        if self
            .inner
            .store
            .load()
            .ok()
            .is_some_and(|s| s.session_token.is_some())
        {
            let _ = self.inner.close();
        }
    }
}
impl Drop for ClientCore {
    fn drop(&mut self) {
        self.heartbeat.stop();
    }
}
impl ClientCore {
    fn set_session(&self, value: Option<String>) {
        if let Ok(mut token) = self.session_guard.lock() {
            *token = value;
        }
    }
    pub fn checkout(&self) -> Result<LicenseResult> {
        let result = self.run(|s| self.seat_inner(s, "checkout"));
        if result.error_code() == Some(ErrorCode::AllSeatsInUse) {
            return Err(Error(ErrorCode::AllSeatsInUse));
        }
        self.start_heartbeat(&result);
        Ok(result)
    }
    pub fn heartbeat(&self) -> LicenseResult {
        self.run(|s| self.seat_inner(s, "heartbeat"))
    }
    pub fn checkin(&self) -> LicenseResult {
        self.heartbeat.stop();
        self.run(|s| self.seat_inner(s, "checkin"))
    }
    pub fn close(&self) -> LicenseResult {
        self.checkin()
    }
    fn start_heartbeat(&self, result: &LicenseResult) {
        if !result.valid() || result.payload["floating"].is_null() {
            return;
        }
        let Some(interval) = result.payload["floating"]["heartbeat_interval_seconds"]
            .as_u64()
            .filter(|n| (30..=600).contains(n))
        else {
            return;
        };
        let signal = self.heartbeat.clone();
        let Ok(mut state) = signal.state.lock() else {
            return;
        };
        if state.1 {
            return;
        }
        state.1 = true;
        let generation = state.0;
        drop(state);
        let weak = self.weak.clone();
        let _ = std::thread::Builder::new()
            .name("kdpika-seat-heartbeat".into())
            .spawn(move || loop {
                let Ok(state) = signal.state.lock() else {
                    return;
                };
                let Ok((state, _)) =
                    signal
                        .wake
                        .wait_timeout_while(state, Duration::from_secs(interval), |s| {
                            s.0 == generation && s.1
                        })
                else {
                    return;
                };
                if state.0 != generation || !state.1 {
                    return;
                }
                drop(state);
                let Some(client) = weak.upgrade() else { return };
                let result = client.heartbeat();
                if !result.valid() && result.error_code() != Some(ErrorCode::Network) {
                    signal.stop();
                    return;
                }
            });
    }
    fn seat_inner(&self, state: &mut State, action: &'static str) -> Result<LicenseResult> {
        let Some(token) = state.token.clone().map(Zeroizing::new) else {
            return Ok(self.denied(ErrorCode::NotActivated, Source::Local, None));
        };
        let session = state.session_token.clone().map(Zeroizing::new);
        if action == "checkin" {
            state.session_token = None;
            state.envelope = None;
            self.set_session(None);
            self.store.save(state)?;
            if session.is_none() {
                let mut r = self.decision(Source::Acknowledgement, None, None);
                r.accepted = true;
                return Ok(r);
            }
        }
        if action == "heartbeat" && session.is_none() {
            return self.seat_inner(state, "checkout");
        }
        let fp = (self.fingerprint)(&self.product)?;
        let path = match action {
            "checkout" => "/api/v1/licenses/checkout",
            "heartbeat" => "/api/v1/licenses/heartbeat",
            _ => "/api/v1/licenses/checkin",
        };
        let reply=match self.request("POST",path,Some(json!({"fingerprint":fp.value(),"session_token":session.as_ref().map(|t|t.as_str())})),Some(&token),false){Ok(r)=>r,Err(e)=>return Ok(self.denied(e.0,Source::Local,None))};
        if !reply.ok() {
            state.session_token = None;
            state.envelope = None;
            self.set_session(None);
            self.store.save(state)?;
            if action == "heartbeat" && reply.code() == ErrorCode::SessionNotFound {
                return self.seat_inner(state, "checkout");
            }
            return Ok(self.denied(reply.code(), Source::Online, Some(&reply)));
        }
        if action == "checkin" {
            let mut r = self.decision(Source::Acknowledgement, None, Some(&reply));
            r.accepted = reply.value["checked_in"] == true;
            if !r.accepted {
                r.error = Some(ErrorCode::InvalidResponse);
            }
            return Ok(r);
        }
        if action == "checkout" {
            let t = reply.value["session_token"]
                .as_str()
                .filter(|t| valid_session(t))
                .ok_or(Error(ErrorCode::InvalidResponse))?;
            state.session_token = Some(t.into());
        }
        self.set_session(state.session_token.clone());
        let env = envelope(&reply)?;
        let r = self.authenticate(state, &env, Source::Online, Some(&reply));
        if r.valid() || r.error_code() == Some(ErrorCode::InsufficientCredits) {
            state.envelope = Some(env);
            self.checkpoint(state, &r)?;
        } else {
            state.session_token = None;
            state.envelope = None;
            self.set_session(None);
            self.store.save(state)?;
        }
        Ok(r)
    }
    pub fn get_balance(&self) -> std::result::Result<u64, CreditError> {
        let _guard = self.store.lock()?;
        let mut state = self.store.load()?;
        let r = self.check_inner(&mut state, true, true)?;
        if !r.signature_valid()
            || (!r.valid() && r.error_code() != Some(ErrorCode::InsufficientCredits))
        {
            return Err(Error(r.error_code().unwrap_or(ErrorCode::InvalidResponse)).into());
        }
        if r.payload["metered"] != true {
            return Err(Error(ErrorCode::NotMeteredLicense).into());
        }
        r.payload["credit_balance"]
            .as_u64()
            .ok_or_else(|| Error(ErrorCode::InvalidResponse).into())
    }
    /// No retry. Reuse a persisted request key after an uncertain transport outcome.
    pub fn consume(
        &self,
        amount: u64,
        key: Option<&str>,
        reason: Option<&str>,
    ) -> std::result::Result<CreditConsumption, CreditError> {
        if amount == 0 || amount > crate::policy::MAX {
            return Err(Error(ErrorCode::InvalidCreditAmount).into());
        }
        if key.is_some_and(|s| {
            s.len() < 8
                || s.len() > 190
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c))
        }) || reason
            .is_some_and(|r| r.chars().count() > 190 || r.chars().any(|c| c.is_ascii_control()))
        {
            return Err(Error(ErrorCode::InvalidRequest).into());
        }
        let _guard = self.store.lock()?;
        let mut state = self.store.load()?;
        let token = state.token.clone().ok_or(Error(ErrorCode::NotActivated))?;
        self.set_session(state.session_token.clone());
        let fp = (self.fingerprint)(&self.product)?;
        let reply=self.request("POST","/api/v1/licenses/consume",Some(json!({"amount":amount,"idempotency_key":key,"reason":reason,"fingerprint":fp.value(),"session_token":state.session_token})),Some(&token),false)?;
        if !reply.ok() && reply.code() != ErrorCode::InsufficientCredits {
            state.envelope = None;
            self.store.save(&state)?;
            return Err(Error(reply.code()).into());
        }
        let env = envelope(&reply)?;
        let r = self.authenticate(&mut state, &env, Source::Online, Some(&reply));
        if !r.signature_valid()
            || (!r.valid() && r.error_code() != Some(ErrorCode::InsufficientCredits))
        {
            return Err(Error(r.error_code().unwrap_or(ErrorCode::SignatureInvalid)).into());
        }
        if r.payload["metered"] != true {
            return Err(Error(ErrorCode::NotMeteredLicense).into());
        }
        if !reply.ok() {
            let balance = r.payload["credit_balance"]
                .as_u64()
                .ok_or(Error(ErrorCode::SignatureInvalid))?;
            if reply.status != 402 || balance >= amount || reply.value["balance"] != balance {
                return Err(Error(ErrorCode::SignatureInvalid).into());
            }
            state.envelope = Some(env);
            self.checkpoint(&mut state, &r)?;
            return Err(CreditError::InsufficientCredits { balance });
        }
        let receipt = &r.payload["credit_transaction"];
        let expected = key.map(|k| verification::hash(k.as_bytes()));
        let id = receipt["id"]
            .as_str()
            .filter(|s| {
                s.starts_with("ctx_")
                    && s.len() <= 40
                    && s.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            })
            .ok_or(Error(ErrorCode::SignatureInvalid))?;
        let balance = receipt["balance_after"]
            .as_u64()
            .filter(|n| *n <= crate::policy::MAX)
            .ok_or(Error(ErrorCode::SignatureInvalid))?;
        if receipt["type"] != "consume"
            || receipt["amount"].as_i64() != Some(-(amount as i64))
            || receipt["idempotency_hash"] != json!(expected)
            || !receipt["total_allocated_after"]
                .as_u64()
                .is_some_and(|v| v <= crate::policy::MAX)
            || !receipt["replayed"].is_boolean()
            || reply.value["transaction_id"] != id
            || reply.value["balance"] != balance
            || reply.value["consumed"] != amount
            || reply.value["replayed"] != receipt["replayed"]
        {
            return Err(Error(ErrorCode::SignatureInvalid).into());
        }
        let result = CreditConsumption {
            transaction_id: id.into(),
            balance,
            consumed: amount,
            replayed: receipt["replayed"].as_bool().unwrap_or(false),
        };
        state.envelope = Some(env);
        self.checkpoint(&mut state, &r)?;
        Ok(result)
    }
    pub fn product(&self) -> &str {
        &self.product
    }
    pub fn environment(&self) -> Environment {
        self.environment
    }
    /// Stable random identity, not a hardware fingerprint or access decision.
    pub fn installation_id(&self) -> &str {
        &self.store.installation
    }
    pub fn storage_security(&self) -> &'static str {
        "permission-protected-file"
    }
    /// Token presence only. Never use this to unlock features.
    pub fn activated(&self) -> Result<bool> {
        let _guard = self.store.lock()?;
        Ok(self.store.load()?.token.is_some())
    }
    /// Activate online. An existing activation must be checked/deactivated, not blindly replaced.
    pub fn activate(&self, key: &str, email: Option<&str>) -> LicenseResult {
        let result = self.run(|state| self.activate_inner(state, key, email));
        self.start_heartbeat(&result);
        result
    }
    /// Use authenticated fresh cache, otherwise validate online.
    pub fn check(&self) -> LicenseResult {
        let result = self.run(|state| self.check_inner(state, false, false));
        self.start_heartbeat(&result);
        result
    }
    /// Try online validation; only an actual network failure can use signed offline grace.
    pub fn validate(&self) -> LicenseResult {
        let result = self.run(|state| self.check_inner(state, true, false));
        self.start_heartbeat(&result);
        result
    }
    /// Require an online response; no offline fallback even on network failure.
    pub fn validate_online(&self) -> LicenseResult {
        let result = self.run(|state| self.check_inner(state, true, true));
        self.start_heartbeat(&result);
        result
    }
    /// Release seat online. Check `deactivated()`, not `valid()` on this acknowledgement.
    pub fn deactivate(&self) -> LicenseResult {
        self.heartbeat.stop();
        self.set_session(None);
        self.run(|state| {
            let Some(token) = &state.token else {
                return Ok(self.denied(ErrorCode::NotActivated, Source::Local, None));
            };
            let reply = match self.request(
                "POST",
                "/api/v1/licenses/deactivate",
                Some(json!({})),
                Some(token),
                false,
            ) {
                Ok(r) => r,
                Err(e) => return Ok(self.denied(e.0, Source::Local, None)),
            };
            if !reply.ok() {
                return Ok(self.denied(reply.code(), Source::Online, Some(&reply)));
            }
            if reply.value["deactivated"] != true {
                return Ok(self.denied(ErrorCode::InvalidResponse, Source::Online, Some(&reply)));
            }
            self.store.clear()?;
            let mut result = self.decision(Source::Acknowledgement, None, Some(&reply));
            result.accepted = true;
            Ok(result)
        })
    }
    pub fn feature(&self, name: &str) -> bool {
        self.check().feature(name)
    }
    pub fn feature_value(&self, name: &str) -> Option<Value> {
        self.check().feature_value(name)
    }
    /// Local recovery only: clears token/cache, keeps identity, DOES NOT release a server seat.
    pub fn reset_local(&self) -> Result<()> {
        self.heartbeat.stop();
        self.set_session(None);
        let _guard = self.store.lock()?;
        self.store.verify_identity()?;
        self.store.clear()
    }
    fn run(&self, operation: impl FnOnce(&mut State) -> Result<LicenseResult>) -> LicenseResult {
        let work = || {
            let _guard = self.store.lock()?;
            let mut state = self.store.load()?;
            operation(&mut state)
        };
        match work() {
            Ok(r) => r,
            Err(e) => self.denied(e.0, Source::Local, None),
        }
    }
    fn activate_inner(
        &self,
        state: &mut State,
        key: &str,
        email: Option<&str>,
    ) -> Result<LicenseResult> {
        if state.token.is_some() {
            return Ok(self.denied(ErrorCode::AlreadyActivated, Source::Local, None));
        }
        if !safe_text(key.trim(), 1024) || email.is_some_and(|e| !safe_text(e.trim(), 254)) {
            return Ok(self.denied(ErrorCode::InvalidLicense, Source::Local, None));
        }
        let fingerprint = (self.fingerprint)(&self.product)?;
        let mut body = json!({"license_key":key.trim(),"device_id":self.store.installation,"device_name":self.application_name,"platform":std::env::consts::OS,"app_version":self.application_version,"fingerprint":fingerprint.value()});
        if let Some(email) = email {
            body["email"] = json!(email.trim());
        }
        let reply = match self.request("POST", "/api/v1/licenses/activate", Some(body), None, true)
        {
            Ok(r) => r,
            Err(e) => return Ok(self.denied(e.0, Source::Local, None)),
        };
        if !reply.ok() {
            return Ok(self.denied(reply.code(), Source::Online, Some(&reply)));
        }
        let token = Zeroizing::new(
            reply.value["activation_token"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        );
        if !storage::valid_token(&token) {
            return Ok(self.denied(ErrorCode::InvalidResponse, Source::Online, Some(&reply)));
        }
        let result = (|| {
            let envelope = envelope(&reply)?;
            let result = self.authenticate(state, &envelope, Source::Online, Some(&reply));
            if !result.valid()
                && !matches!(
                    result.error_code(),
                    Some(ErrorCode::SessionRequired | ErrorCode::InsufficientCredits)
                )
            {
                return Ok(result);
            }
            state.token = Some(token.to_string());
            state.envelope = Some(envelope);
            self.checkpoint(state, &result)?;
            Ok(result)
        })();
        match result {
            Ok(r)
                if r.valid()
                    || matches!(
                        r.error_code(),
                        Some(ErrorCode::SessionRequired | ErrorCode::InsufficientCredits)
                    ) =>
            {
                if r.error_code() == Some(ErrorCode::SessionRequired) {
                    self.seat_inner(state, "checkout")
                } else {
                    Ok(r)
                }
            }
            other => {
                self.release_untrusted(&token);
                other
            }
        }
    }
    fn check_inner(
        &self,
        state: &mut State,
        force: bool,
        strict_online: bool,
    ) -> Result<LicenseResult> {
        let Some(token) = state.token.clone().map(Zeroizing::new) else {
            return Ok(self.denied(ErrorCode::NotActivated, Source::Local, None));
        };
        let fingerprint = (self.fingerprint)(&self.product)?;
        self.set_session(state.session_token.clone());
        if !force {
            if let Some(envelope) = state.envelope.clone() {
                let result = self.authenticate(state, &envelope, Source::Cache, None);
                if result.valid()
                    && (self.clock)() < timestamp(&result.payload["next_validation_at"])?
                {
                    self.checkpoint(state, &result)?;
                    return Ok(result);
                }
            }
        }
        let reply = match self.request(
            "POST",
            "/api/v1/licenses/validate",
            Some(json!({"fingerprint":fingerprint.value(),"session_token":state.session_token})),
            Some(&token),
            false,
        ) {
            Ok(r) => r,
            Err(Error(ErrorCode::Network)) if !strict_online => {
                let Some(envelope) = state.envelope.clone() else {
                    return Ok(self.denied(ErrorCode::OfflineCacheMissing, Source::Offline, None));
                };
                let result = self.authenticate(state, &envelope, Source::Offline, None);
                if result.valid() {
                    self.checkpoint(state, &result)?;
                    return Ok(result);
                }
                if result.error_code() == Some(ErrorCode::SignatureInvalid) {
                    state.envelope = None;
                    self.store.save(state)?;
                    return Ok(self.denied(ErrorCode::OfflineCacheInvalid, Source::Offline, None));
                }
                return Ok(result);
            }
            Err(e) => {
                if e.0 != ErrorCode::Network {
                    state.envelope = None;
                    self.store.save(state)?;
                }
                return Ok(self.denied(e.0, Source::Online, None));
            }
        };
        if !reply.ok() {
            let code = reply.code();
            if reply.status == 402 && code == ErrorCode::InsufficientCredits {
                let env = envelope(&reply)?;
                let checked = self.authenticate(state, &env, Source::Online, Some(&reply));
                if checked.signature_valid()
                    && checked.error_code() == Some(ErrorCode::InsufficientCredits)
                {
                    state.envelope = Some(env);
                    self.checkpoint(state, &checked)?;
                    return Ok(checked);
                }
                return Ok(self.denied(ErrorCode::SignatureInvalid, Source::Online, Some(&reply)));
            }
            if code == ErrorCode::SessionNotFound {
                state.session_token = None;
                self.set_session(None);
                return self.seat_inner(state, "checkout");
            }
            state.envelope = None;
            state.session_token = None;
            self.set_session(None);
            if matches!(
                code,
                ErrorCode::InvalidToken
                    | ErrorCode::DeviceMismatch
                    | ErrorCode::FingerprintMismatch
                    | ErrorCode::LicenseRevoked
                    | ErrorCode::LicenseExpired
                    | ErrorCode::LicenseCancelled
                    | ErrorCode::LicenseSuspended
            ) {
                state.token = None;
            }
            self.store.save(state)?;
            return Ok(self.denied(code, Source::Online, Some(&reply)));
        }
        let envelope = match envelope(&reply) {
            Ok(s) => s,
            Err(e) => {
                state.envelope = None;
                self.store.save(state)?;
                return Ok(self.denied(e.0, Source::Online, Some(&reply)));
            }
        };
        let result = self.authenticate(state, &envelope, Source::Online, Some(&reply));
        if result.valid() {
            state.envelope = Some(envelope);
            self.checkpoint(state, &result)?;
        } else {
            state.envelope = None;
            self.store.save(state)?;
        }
        Ok(result)
    }
    fn authenticate(
        &self,
        state: &mut State,
        envelope: &str,
        source: Source,
        reply: Option<&Reply>,
    ) -> LicenseResult {
        let mut verify = || -> Result<Value> {
            let env = json::parse(envelope.as_bytes())?;
            let target = json::field(&env, "key_id")?;
            let key = if let Some(pin) = &self.pin {
                if pin.key_id() != target {
                    return Err(Error(ErrorCode::KeyRotationRequired));
                }
                pin.clone()
            } else if let Some(key) = state
                .verification_key
                .as_ref()
                .filter(|k| k.key_id() == target)
            {
                key.clone()
            } else {
                if source != Source::Online {
                    return Err(Error(ErrorCode::SignatureInvalid));
                }
                {
                    let mut last = self
                        .last_key_fetch
                        .lock()
                        .map_err(|_| Error(ErrorCode::SignatureInvalid))?;
                    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(10)) {
                        return Err(Error(ErrorCode::SignatureInvalid));
                    }
                    *last = Some(Instant::now());
                }
                let response = self.request("GET", "/api/v1/public-key", None, None, false)?;
                if !response.ok() {
                    return Err(Error(ErrorCode::SignatureInvalid));
                }
                let raw = json::parse(&response.raw)?;
                let value = raw.0.get("key").ok_or(Error(ErrorCode::SignatureInvalid))?;
                let key = VerificationKey::from_json(value.get().as_bytes())?;
                if key.key_id() != target {
                    return Err(Error(ErrorCode::SignatureInvalid));
                }
                key
            };
            let payload = verification::verify(&key, envelope)?;
            state.verification_key = Some(key);
            Ok(payload)
        };
        let payload = match verify() {
            Ok(p) => p,
            Err(e) => {
                return self.denied(
                    if e.0 == ErrorCode::KeyRotationRequired {
                        e.0
                    } else {
                        ErrorCode::SignatureInvalid
                    },
                    source,
                    reply,
                )
            }
        };
        if payload["product_id"] != self.product
            || payload["environment"]
                .as_str()
                .map(str::to_ascii_uppercase)
                .as_deref()
                != Some(self.environment.as_str())
            || payload["device"]["installation_hash"]
                != verification::hash(self.store.installation.as_bytes())
            || !payload["features"].is_object()
        {
            return self.denied(ErrorCode::SignatureInvalid, source, reply);
        }
        let fingerprint = match (self.fingerprint)(&self.product) {
            Ok(v) => v,
            Err(e) => return self.denied(e.0, source, reply),
        };
        if !crate::fingerprint::matches(&payload["device"]["fingerprint"], &fingerprint) {
            return self.denied(ErrorCode::FingerprintMismatch, source, reply);
        }
        let times = match Times::parse(&payload) {
            Ok(t) => t,
            Err(e) => return self.denied(e.0, source, reply),
        };
        let checkpoint = match &state.checkpoint {
            Some(t) => match timestamp(&json!(t)) {
                Ok(t) => t,
                Err(e) => return self.denied(e.0, source, reply),
            },
            None => times.server,
        };
        let now = (self.clock)();
        let refusal = if now + TimeDelta::seconds(300) < checkpoint.max(times.server) {
            Some(ErrorCode::ClockRollbackDetected)
        } else if payload["status"] != "active" {
            Some(ErrorCode::LicenseNotActive)
        } else if times.starts.is_some_and(|t| now < t) {
            Some(ErrorCode::LicenseNotStarted)
        } else if times.expiry.is_some_and(|t| now >= t) {
            Some(ErrorCode::LicenseExpired)
        } else if matches!(source, Source::Cache | Source::Offline) && now > times.grace {
            Some(ErrorCode::OfflineGraceExpired)
        } else {
            None
        };
        if let Some(code) = refusal {
            return self.denied(code, source, reply);
        }
        let policy = || -> Option<ErrorCode> {
            if !payload["floating"].is_null() {
                let f = &payload["floating"];
                if !f["heartbeat_interval_seconds"]
                    .as_u64()
                    .is_some_and(|v| (30..=600).contains(&v))
                {
                    return Some(ErrorCode::InvalidResponse);
                }
                if f["session"].is_null() {
                    return Some(ErrorCode::SessionRequired);
                }
            }
            if !crate::policy::lease(&payload, state.session_token.as_deref(), now) {
                return Some(ErrorCode::SessionExpired);
            }
            if !crate::policy::credits(&payload) {
                return Some(ErrorCode::InvalidResponse);
            }
            if payload["metered"] == true {
                if source != Source::Online {
                    return Some(ErrorCode::MeteredOnlineRequired);
                }
                if payload["credit_balance"] == 0 {
                    return Some(ErrorCode::InsufficientCredits);
                }
            }
            None
        };
        if let Some(code) = policy() {
            let mut result = self.decision(source, Some(code), reply);
            result.payload = payload;
            result.signature = true;
            return result;
        }
        let mut result = self.decision(source, None, reply);
        result.accepted = true;
        result.signature = true;
        result.payload = payload;
        result
    }
    fn checkpoint(&self, state: &mut State, result: &LicenseResult) -> Result<()> {
        let server = timestamp(&result.payload["server_time"])?;
        let previous = state
            .checkpoint
            .as_ref()
            .map(|v| timestamp(&json!(v)))
            .transpose()?
            .unwrap_or(server);
        state.checkpoint = Some((self.clock)().max(server).max(previous).to_rfc3339());
        self.store.save(state)
    }
    fn release_untrusted(&self, token: &str) {
        let _ = self.request(
            "POST",
            "/api/v1/licenses/deactivate",
            Some(json!({})),
            Some(token),
            false,
        );
    }
    fn request(
        &self,
        method: &'static str,
        path: &'static str,
        body: Option<Value>,
        token: Option<&str>,
        product_headers: bool,
    ) -> Result<Reply> {
        let mut headers = BTreeMap::new();
        if product_headers {
            headers.insert("X-App-ID".into(), self.product.clone());
            headers.insert("X-Environment".into(), self.environment.as_str().into());
        }
        if let Some(token) = token {
            headers.insert("Authorization".into(), format!("Bearer {token}"));
            headers.insert("X-Installation-ID".into(), self.store.installation.clone());
        }
        let response: HttpResponse = self
            .transport
            .request(Request {
                method,
                path,
                headers,
                body: body
                    .map(|b| serde_json::to_vec(&b))
                    .transpose()
                    .map_err(|_| Error(ErrorCode::InvalidRequest))?,
            })
            .map_err(|_| Error(ErrorCode::Network))?;
        let value = match json::value(&response.body) {
            Ok(v) => v,
            Err(_) => json!({"success":false,"error":{"code":"INVALID_RESPONSE"}}),
        };
        let id = value["request_id"]
            .as_str()
            .filter(|id| {
                id.starts_with("req_")
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
            .map(String::from);
        Ok(Reply {
            status: response.status,
            value,
            raw: response.body,
            id,
            retry: response
                .retry_after
                .filter(|s| s.len() <= 128 && s.bytes().all(|c| (32..127).contains(&c))),
        })
    }
    fn decision(
        &self,
        source: Source,
        error: Option<ErrorCode>,
        reply: Option<&Reply>,
    ) -> LicenseResult {
        LicenseResult {
            accepted: false,
            signature: false,
            source,
            payload: Value::Null,
            error,
            request_id: reply.and_then(|r| r.id.clone()),
            retry_after: reply.and_then(|r| r.retry.clone()),
            clock: self.clock.clone(),
            checked_at: (self.clock)(),
            monotonic: Instant::now(),
            session_guard: self.session_guard.clone(),
        }
    }
    fn denied(&self, code: ErrorCode, source: Source, reply: Option<&Reply>) -> LicenseResult {
        self.decision(source, Some(code), reply)
    }
}
fn envelope(reply: &Reply) -> Result<String> {
    let raw = json::parse(&reply.raw)?;
    let env = raw
        .0
        .get("license")
        .ok_or(Error(ErrorCode::InvalidResponse))?;
    if !env.get().starts_with('{') {
        return Err(Error(ErrorCode::InvalidResponse));
    }
    Ok(env.get().to_string())
}
fn safe_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn valid_session(t: &str) -> bool {
    t.len() == 47
        && t.starts_with("flt_")
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
