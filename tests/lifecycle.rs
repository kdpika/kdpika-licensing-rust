use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};
use kdpika_licensing::{
    Environment, ErrorCode, HttpResponse, LicenseClient, NetworkError, Request, Source, Transport,
    VerificationKey,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc, Mutex,
};
const NOW: i64 = 1_792_800_000;
const APP: &str = "app_rust_test";
const TOKEN: &str = "act_SYNTHETIC_TEST_ONLY";
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn key() -> (SigningKey, Value) {
    let key = SigningKey::from_bytes(&[7; 32]);
    let public = key.verifying_key().to_bytes();
    let doc = json!({"algorithm":"Ed25519","encoding":"base64url","public_key":URL_SAFE_NO_PAD.encode(public),"key_id":format!("ed25519_{}",&hash(&public)[..16])});
    (key, doc)
}
fn iso(time: i64) -> String {
    DateTime::<Utc>::from_timestamp(time, 0)
        .unwrap()
        .to_rfc3339()
}
#[derive(Default)]
struct Mode {
    offline: bool,
    status: Option<u16>,
    code: Option<&'static str>,
    tamper: bool,
    wrong_product: bool,
    wrong_device: bool,
    wrong_environment: bool,
    expires: Option<i64>,
    lifetime: bool,
    starts: Option<i64>,
    missing_expiry: bool,
    bad_time: bool,
    missing_license: bool,
    duplicate: bool,
    wrong_key: bool,
    failed_release: bool,
    requests: Vec<&'static str>,
    installation: String,
    fingerprint: Value,
}
struct Mock {
    mode: Mutex<Mode>,
    now: Arc<AtomicI64>,
}
impl Transport for Mock {
    fn request(&self, r: Request) -> std::result::Result<HttpResponse, NetworkError> {
        let mut m = self.mode.lock().unwrap();
        m.requests.push(r.path);
        if m.offline {
            return Err(NetworkError);
        }
        if let Some(status) = m.status {
            return Ok(HttpResponse{status,body:serde_json::to_vec(&json!({"success":false,"error":{"code":m.code.unwrap_or("UNKNOWN_FROM_SERVER")},"request_id":"req_test_42"})).unwrap(),retry_after:Some("60".into())});
        }
        if r.path.ends_with("public-key") {
            return Ok(HttpResponse {
                status: 200,
                body: serde_json::to_vec(&json!({"success":true,"key":key().1})).unwrap(),
                retry_after: None,
            });
        }
        if r.path.ends_with("deactivate") {
            assert_eq!(
                r.headers.get("Authorization").map(String::as_str),
                Some("Bearer act_SYNTHETIC_TEST_ONLY")
            );
            return Ok(HttpResponse {
                status: 200,
                body: serde_json::to_vec(&json!({"success":true,"deactivated":!m.failed_release}))
                    .unwrap(),
                retry_after: None,
            });
        }
        if r.path.ends_with("activate") {
            assert_eq!(r.headers.get("X-App-ID").map(String::as_str), Some(APP));
            assert_eq!(
                r.headers.get("X-Environment").map(String::as_str),
                Some("SANDBOX")
            );
            let body: Value = serde_json::from_slice(r.body.as_ref().unwrap()).unwrap();
            m.installation = body["device_id"].as_str().unwrap().into();
            m.fingerprint = body["fingerprint"].clone();
            m.fingerprint["policy"] = json!({"mode":"fuzzy","min_matches":2});
            if body["email"] != "customer@example.test" {
                return Ok(HttpResponse {
                    status: 422,
                    body: br#"{"success":false,"error":{"code":"INVALID_LICENSE"}}"#.to_vec(),
                    retry_after: None,
                });
            }
        } else {
            assert_eq!(
                r.headers.get("X-Installation-ID").map(String::as_str),
                Some(m.installation.as_str())
            );
        }
        let now = self.now.load(Ordering::SeqCst);
        let mut payload = json!({"product_id":APP,"environment":"sandbox","license_id":"lic_synthetic","status":"active","server_time":iso(now),"next_validation_at":iso(now+3600),"offline_grace_until":iso(now+7*86400),"expires_at":if m.lifetime{Value::Null}else{json!(iso(m.expires.unwrap_or(now+30*86400)))},"device":{"installation_hash":hash(m.installation.as_bytes())},"features":{"pdf_export":true,"seat_count":3,"negative":-2,"fraction":2.5,"word":"true","off":false,"nested":{"level":"pro"}}});
        payload["device"]["fingerprint"] = m.fingerprint.clone();
        payload["floating"] = Value::Null;
        payload["metered"] = json!(false);
        payload["credit_balance"] = json!(0);
        payload["total_allocated"] = json!(0);
        payload["credit_transaction"] = Value::Null;
        if m.wrong_product {
            // The current wire contract always carries hardware, seats and credit policy.
            payload["product_id"] = json!("app_other");
        }
        if m.wrong_device {
            payload["device"]["installation_hash"] = json!("other");
        }
        if m.wrong_environment {
            payload["environment"] = json!("live");
        }
        if let Some(t) = m.starts {
            payload["starts_at"] = json!(iso(t));
        }
        if m.missing_expiry {
            payload.as_object_mut().unwrap().remove("expires_at");
        }
        if m.bad_time {
            payload["offline_grace_until"] = json!(iso(now - 1));
        }
        let (signer, doc) = key();
        let encoded = serde_json::to_string(&payload).unwrap();
        let signature = URL_SAFE_NO_PAD.encode(signer.sign(encoded.as_bytes()).to_bytes());
        let mut envelope = json!({"payload":payload,"payload_json":encoded,"signature":signature,"algorithm":"Ed25519","key_id":doc["key_id"]});
        if m.tamper {
            envelope["payload"]["features"]["pdf_export"] = json!(false);
        }
        if m.wrong_key {
            envelope["key_id"] = json!("ed25519_other");
        }
        let mut reply = json!({"success":true,"activation_token":TOKEN,"license":envelope,"request_id":"req_test_42"});
        if m.missing_license {
            reply.as_object_mut().unwrap().remove("license");
        }
        let body = if m.duplicate {
            br#"{"success":true,"success":false}"#.to_vec()
        } else {
            serde_json::to_vec(&reply).unwrap()
        };
        Ok(HttpResponse {
            status: 200,
            body,
            retry_after: None,
        })
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    now: Arc<AtomicI64>,
    transport: Arc<Mock>,
    client: LicenseClient,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let now = Arc::new(AtomicI64::new(NOW));
        let transport = Arc::new(Mock {
            mode: Mutex::new(Mode::default()),
            now: now.clone(),
        });
        let client = make(dir.path(), now.clone(), transport.clone(), true);
        Self {
            dir,
            now,
            transport,
            client,
        }
    }
    fn activate(&self) -> kdpika_licensing::LicenseResult {
        self.client
            .activate("TEST-SYNTHETIC", Some("customer@example.test"))
    }
    fn mode(&self, update: impl FnOnce(&mut Mode)) {
        update(&mut self.transport.mode.lock().unwrap());
    }
    fn advance(&self, seconds: i64) {
        self.now.fetch_add(seconds, Ordering::SeqCst);
    }
}
fn make(path: &std::path::Path, now: Arc<AtomicI64>, t: Arc<Mock>, pinned: bool) -> LicenseClient {
    let mut b = LicenseClient::builder(APP)
        .storage_dir(path)
        .environment(Environment::Sandbox)
        .transport(t)
        .clock(move || DateTime::<Utc>::from_timestamp(now.load(Ordering::SeqCst), 0).unwrap());
    if pinned {
        b = b.verification_key(
            VerificationKey::from_json(&serde_json::to_vec(&key().1).unwrap()).unwrap(),
        );
    }
    b.build().unwrap()
}
#[test]
fn lifecycle_and_fail_closed_features() {
    let f = Fixture::new();
    assert_eq!(f.client.check().error_code(), Some(ErrorCode::NotActivated));
    let r = f.activate();
    assert!(r.valid() && r.signature_valid());
    assert!(r.feature("pdf_export"));
    assert_eq!(r.limit("seat_count"), Some(3));
    assert_eq!(r.limit("negative"), None);
    assert_eq!(r.limit("fraction"), None);
    assert!(!r.feature("word"));
    assert!(!r.feature("missing"));
    assert!(r.feature("fraction"));
    assert_eq!(r.feature_value("nested"), Some(json!({"level":"pro"})));
    assert_eq!(
        f.client.activate("OTHER", None).error_code(),
        Some(ErrorCode::AlreadyActivated)
    );
    assert_eq!(f.client.check().source(), Source::Cache);
    assert_eq!(f.client.validate().source(), Source::Online);
    let ack = f.client.deactivate();
    assert!(ack.deactivated());
    assert!(!ack.valid() && !ack.signature_valid() && !ack.feature("pdf_export"));
    assert!(!f.client.activated().unwrap());
}
#[test]
fn wrong_email_is_not_activated() {
    let f = Fixture::new();
    let r = f
        .client
        .activate("TEST-SYNTHETIC", Some("other@example.test"));
    assert_eq!(r.error_code(), Some(ErrorCode::InvalidLicense));
    assert!(!f.client.activated().unwrap());
}
#[test]
fn installation_and_cache_persist_across_clients() {
    let f = Fixture::new();
    assert!(f.activate().valid());
    let other = make(f.dir.path(), f.now.clone(), f.transport.clone(), true);
    assert_eq!(f.client.installation_id(), other.installation_id());
    let before = f.transport.mode.lock().unwrap().requests.len();
    assert!(other.check().valid());
    assert_eq!(before, f.transport.mode.lock().unwrap().requests.len());
}
#[test]
fn network_loss_uses_only_bounded_signed_grace() {
    let f = Fixture::new();
    assert!(f.activate().valid());
    f.advance(3700);
    f.mode(|m| m.offline = true);
    let r = f.client.check();
    assert!(r.valid() && r.source() == Source::Offline);
    assert_eq!(
        f.client.validate_online().error_code(),
        Some(ErrorCode::Network)
    );
    f.advance(7 * 86400);
    assert_eq!(
        f.client.check().error_code(),
        Some(ErrorCode::OfflineGraceExpired)
    );
}
#[test]
fn expiry_wins_over_grace() {
    let f = Fixture::new();
    f.mode(|m| m.expires = Some(NOW + 4000));
    assert!(f.activate().valid());
    f.advance(4000);
    f.mode(|m| m.offline = true);
    let r = f.client.validate();
    assert_eq!(r.error_code(), Some(ErrorCode::LicenseExpired));
    assert!(!r.feature("pdf_export"));
}
#[test]
fn held_results_expire_and_cannot_be_mutated() {
    let f = Fixture::new();
    let r = f.activate();
    let mut value = r.feature_value("nested").unwrap();
    value["level"] = json!("forged");
    assert_eq!(r.feature_value("nested").unwrap()["level"], "pro");
    f.advance(3600);
    assert!(!r.valid() && !r.feature("pdf_export"));
}
#[test]
fn authoritative_http_refusals_never_fall_back() {
    for status in [302, 307, 401, 403, 429, 500, 503] {
        let f = Fixture::new();
        assert!(f.activate().valid());
        f.mode(|m| {
            m.status = Some(status);
            m.code = Some("LICENSE_REVOKED");
        });
        let r = f.client.validate();
        assert!(!r.valid() && !r.feature("pdf_export"));
        assert_eq!(r.request_id(), Some("req_test_42"));
        assert_eq!(r.retry_after(), Some("60"));
        f.mode(|m| {
            m.status = None;
            m.offline = true;
        });
        assert!(!f.client.check().valid());
    }
}
#[test]
fn tampered_malformed_or_wrong_binding_success_clears_old_grant() {
    for mode in 0..7 {
        let f = Fixture::new();
        assert!(f.activate().valid());
        f.mode(|m| match mode {
            0 => m.tamper = true,
            1 => m.wrong_product = true,
            2 => m.wrong_device = true,
            3 => m.wrong_environment = true,
            4 => m.missing_license = true,
            5 => m.duplicate = true,
            _ => m.bad_time = true,
        });
        assert!(!f.client.validate().valid());
        f.mode(|m| m.offline = true);
        assert!(!f.client.validate().valid());
    }
}
#[test]
fn invalid_activation_is_released_without_saving_token() {
    let f = Fixture::new();
    f.mode(|m| m.tamper = true);
    assert_eq!(f.activate().error_code(), Some(ErrorCode::SignatureInvalid));
    assert!(!f.client.activated().unwrap());
    assert!(f
        .transport
        .mode
        .lock()
        .unwrap()
        .requests
        .contains(&"/api/v1/licenses/deactivate"));
}
#[test]
fn rollback_and_future_start_are_denials() {
    let f = Fixture::new();
    assert!(f.activate().valid());
    f.advance(-301);
    f.mode(|m| m.offline = true);
    assert_eq!(
        f.client.validate().error_code(),
        Some(ErrorCode::ClockRollbackDetected)
    );
    let f = Fixture::new();
    f.mode(|m| m.starts = Some(NOW + 600));
    assert_eq!(
        f.activate().error_code(),
        Some(ErrorCode::LicenseNotStarted)
    );
}
#[test]
fn lifetime_requires_explicit_null_expiry() {
    let f = Fixture::new();
    f.mode(|m| m.lifetime = true);
    let r = f.activate();
    assert!(r.valid() && r.expires_at().is_none());
    let f = Fixture::new();
    f.mode(|m| m.missing_expiry = true);
    assert_eq!(
        f.activate().error_code(),
        Some(ErrorCode::InvalidSignedTime)
    );
}
#[test]
fn key_is_fetched_only_from_configured_transport() {
    let dir = tempfile::tempdir().unwrap();
    let now = Arc::new(AtomicI64::new(NOW));
    let t = Arc::new(Mock {
        mode: Mutex::new(Mode::default()),
        now: now.clone(),
    });
    let c = make(dir.path(), now, t.clone(), false);
    assert!(c.activate("TEST", Some("customer@example.test")).valid());
    assert_eq!(
        t.mode
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|p| p.ends_with("public-key"))
            .count(),
        1
    );
    assert!(c.validate().valid());
    assert_eq!(
        t.mode
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|p| p.ends_with("public-key"))
            .count(),
        1
    );
}
#[test]
fn pin_rotation_is_not_silently_accepted() {
    let f = Fixture::new();
    f.mode(|m| m.wrong_key = true);
    assert_eq!(
        f.activate().error_code(),
        Some(ErrorCode::KeyRotationRequired)
    );
}
#[test]
fn failed_release_preserves_recovery_state_and_identity() {
    let f = Fixture::new();
    assert!(f.activate().valid());
    f.mode(|m| m.failed_release = true);
    assert!(!f.client.deactivate().deactivated());
    assert!(f.client.activated().unwrap());
    let id = f.client.installation_id().to_string();
    f.client.reset_local().unwrap();
    assert_eq!(f.client.installation_id(), id);
    assert!(!f.client.activated().unwrap());
}
#[test]
fn namespaces_are_product_origin_and_environment_scoped() {
    let f = Fixture::new();
    let live = LicenseClient::builder(APP)
        .environment(Environment::Live)
        .storage_dir(f.dir.path())
        .build()
        .unwrap();
    assert_ne!(live.installation_id(), f.client.installation_id());
    let other = LicenseClient::builder("app_other")
        .storage_dir(f.dir.path())
        .build()
        .unwrap();
    assert_ne!(other.installation_id(), f.client.installation_id());
}
#[test]
fn debug_and_errors_do_not_include_private_data() {
    let f = Fixture::new();
    let r = f.activate();
    let debug = format!("{:?} {:?}", f.client, r);
    for private in [
        TOKEN,
        "customer@example.test",
        "TEST-SYNTHETIC",
        f.dir.path().to_str().unwrap(),
        f.client.installation_id(),
    ] {
        assert!(!debug.contains(private));
    }
}
#[test]
fn unsafe_origins_are_rejected() {
    for origin in [
        "http://example.com",
        "https://user:pass@example.com",
        "https://example.com/api/v1",
        "https://example.com?x=1",
        "https://example.com#x",
        "file:///etc/passwd",
        "https://example.com\\bad",
    ] {
        assert!(LicenseClient::builder(APP)
            .base_url(origin)
            .build()
            .is_err());
    }
    assert!(LicenseClient::builder(APP)
        .storage_dir("relative")
        .build()
        .is_err());
}
#[test]
fn corrupt_cache_never_grants_access() {
    let f = Fixture::new();
    assert!(f.activate().valid());
    let namespace = std::fs::read_dir(f.dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(
        namespace.join("state.json"),
        b"{\"token\":\"act_bad\",\"token\":\"act_also\"}",
    )
    .unwrap();
    assert_eq!(f.client.check().error_code(), Some(ErrorCode::Storage));
    let identity = f.client.installation_id().to_owned();
    f.client.reset_local().unwrap();
    assert_eq!(f.client.installation_id(), identity);
    assert_eq!(f.client.check().error_code(), Some(ErrorCode::NotActivated));
}
#[cfg(unix)]
#[test]
fn unix_files_are_private_and_symlink_reads_fail() {
    use std::os::unix::fs::{symlink, MetadataExt};
    let f = Fixture::new();
    assert!(f.activate().valid());
    let namespace = std::fs::read_dir(f.dir.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(std::fs::metadata(&namespace).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        std::fs::metadata(namespace.join("state.json"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
    std::fs::remove_file(namespace.join("state.json")).unwrap();
    symlink(f.dir.path().join("other"), namespace.join("state.json")).unwrap();
    assert_eq!(f.client.check().error_code(), Some(ErrorCode::Storage));
}
#[test]
fn concurrent_activation_is_serialized_and_never_blindly_repeated() {
    let f = Fixture::new();
    let client = Arc::new(f.client);
    let handles: Vec<_> = (0..5)
        .map(|_| {
            let c = client.clone();
            std::thread::spawn(move || c.activate("TEST", Some("customer@example.test")))
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.valid()).count(), 1);
    assert_eq!(
        f.transport
            .mode
            .lock()
            .unwrap()
            .requests
            .iter()
            .filter(|p| p.ends_with("activate") && !p.ends_with("deactivate"))
            .count(),
        1
    );
}
#[cfg(feature = "async")]
#[tokio::test]
async fn async_worker_does_not_block_or_drop_http_on_tokio() {
    let dir = tempfile::tempdir().unwrap();
    let client = kdpika_licensing::AsyncLicenseClient::build(
        LicenseClient::builder(APP).storage_dir(dir.path()),
    )
    .await
    .unwrap();
    let result = client.check().await.unwrap();
    assert_eq!(result.error_code(), Some(ErrorCode::NotActivated));
    drop(client);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
}
