//! Focused B/C/D contract checks only; synthetic hardware, transport, clock and signatures.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey};
use kdpika_licensing::{
    CreditError, Environment, ErrorCode, HardwareFingerprint, HttpResponse, LicenseClient,
    NetworkError, Request, Transport, VerificationKey,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Mutex,
    },
};
const APP: &str = "app_bcd_fixture";
const NOW: i64 = 1792800000;
fn hash(v: &str) -> String {
    format!("{:x}", Sha256::digest(v.as_bytes()))
}
fn iso(v: i64) -> String {
    DateTime::<Utc>::from_timestamp(v, 0).unwrap().to_rfc3339()
}
fn hardware(nic: &str, board: &str) -> HardwareFingerprint {
    HardwareFingerprint::from_components(
        APP,
        &BTreeMap::from([
            ("baseboard".into(), board.into()),
            ("cpu".into(), "fixture-cpu".into()),
            ("nic".into(), nic.into()),
        ]),
        Some(true),
    )
    .unwrap()
}
struct Mode {
    fp: Value,
    installation: String,
    floating: bool,
    held: bool,
    full: bool,
    missing: bool,
    offline: bool,
    metered: bool,
    balance: u64,
    ledger: BTreeMap<String, Value>,
    calls: Vec<String>,
    tamper: bool,
    wrong_tenant: bool,
    bad_fingerprint: bool,
    missing_policy: bool,
    lost: bool,
}
impl Default for Mode {
    fn default() -> Self {
        Self {
            fp: Value::Null,
            installation: String::new(),
            floating: false,
            held: false,
            full: false,
            missing: false,
            offline: false,
            metered: false,
            balance: 5,
            ledger: BTreeMap::new(),
            calls: vec![],
            tamper: false,
            wrong_tenant: false,
            bad_fingerprint: false,
            missing_policy: false,
            lost: false,
        }
    }
}
struct Mock {
    mode: Mutex<Mode>,
    now: Arc<AtomicI64>,
}
fn reply(value: Value, status: u16) -> Result<HttpResponse, NetworkError> {
    Ok(HttpResponse {
        status,
        body: serde_json::to_vec(&value).unwrap(),
        retry_after: None,
    })
}
impl Transport for Mock {
    fn request(&self, r: Request) -> Result<HttpResponse, NetworkError> {
        let mut m = self.mode.lock().unwrap();
        m.calls.push(r.path.into());
        if m.offline {
            return Err(NetworkError);
        }
        if r.path.ends_with("/public-key") {
            let signer = SigningKey::from_bytes(&[9; 32]);
            let public = signer.verifying_key().to_bytes();
            return reply(
                json!({"success":true,"key":{"algorithm":"Ed25519","encoding":"base64url","public_key":B64.encode(public),"key_id":format!("ed25519_{}",&format!("{:x}",Sha256::digest(public))[..16])}}),
                200,
            );
        }
        let body: Value = serde_json::from_slice(r.body.as_deref().unwrap_or(b"null")).unwrap();
        if r.path.ends_with("/activate") {
            m.installation = body["device_id"].as_str().unwrap().into();
            assert_eq!(body["fingerprint"]["version"], 1);
            m.fp = body["fingerprint"].clone();
        } else {
            assert_eq!(r.headers.get("X-Installation-ID").unwrap(), &m.installation);
            if !r.path.ends_with("/deactivate") {
                assert!(body["fingerprint"].is_object());
            }
        }
        if r.path.ends_with("/deactivate") {
            return reply(json!({"success":true,"deactivated":true}), 200);
        }
        if r.path.ends_with("/checkout") {
            if m.full {
                return reply(
                    json!({"success":false,"error":{"code":"ALL_SEATS_IN_USE"}}),
                    409,
                );
            }
            m.held = true;
        }
        if r.path.ends_with("/heartbeat") && m.missing {
            m.missing = false;
            m.held = false;
            return reply(
                json!({"success":false,"error":{"code":"SESSION_NOT_FOUND"}}),
                409,
            );
        }
        if r.path.ends_with("/checkin") {
            m.held = false;
            return reply(json!({"success":true,"checked_in":true}), 200);
        }
        let mut status = 200;
        let mut code = None;
        let mut receipt = Value::Null;
        if r.path.ends_with("/validate") && m.floating && !m.held {
            return reply(
                json!({"success":false,"error":{"code":"SESSION_NOT_FOUND"}}),
                409,
            );
        }
        if r.path.ends_with("/consume") {
            let n = body["amount"].as_u64().unwrap();
            let key = body["idempotency_key"].as_str().map(str::to_string);
            if let Some(old) = key.as_ref().and_then(|k| m.ledger.get(k)).cloned() {
                if old["amount"] != -(n as i64) {
                    return reply(
                        json!({"success":false,"error":{"code":"IDEMPOTENCY_CONFLICT"}}),
                        409,
                    );
                }
                receipt = old;
                receipt["replayed"] = json!(true);
            } else if m.balance < n {
                status = 402;
                code = Some("INSUFFICIENT_CREDITS");
            } else {
                m.balance -= n;
                receipt = json!({"id":format!("ctx_fixture_{}",m.ledger.len()),"type":"consume","amount":-(n as i64),"balance_after":m.balance,"total_allocated_after":5,"idempotency_hash":key.as_ref().map(|k|hash(k)),"replayed":false});
                if let Some(k) = key {
                    m.ledger.insert(k, receipt.clone());
                }
                if m.lost {
                    m.lost = false;
                    return Err(NetworkError);
                }
            }
        }
        if r.path.ends_with("/validate") && m.metered && m.balance == 0 {
            status = 402;
            code = Some("INSUFFICIENT_CREDITS");
        }
        let now = self.now.load(Ordering::SeqCst);
        let seat = format!("flt_{}", "A".repeat(43));
        let mut fp = m.fp.clone();
        fp["policy"] = json!({"mode":"fuzzy","min_matches":2});
        if m.bad_fingerprint {
            fp["components"]["baseboard"] = json!(hash("wrong"));
        }
        let mut payload = json!({"product_id":if m.wrong_tenant{"app_foreign"}else{APP},"environment":"sandbox","status":"active","expires_at":null,"features":{"export":true},"device":{"installation_hash":hash(&m.installation),"fingerprint":fp},"server_time":iso(now),"next_validation_at":iso(now+3600),"offline_grace_until":iso(now+86400),"floating":if m.floating{json!({"max_concurrent_sessions":1,"heartbeat_interval_seconds":180,"session_timeout_seconds":600,"session":if m.held{json!({"id":"ses_fixture","token_hash":hash(&seat),"expires_at":iso(now+600)})}else{Value::Null}})}else{Value::Null},"metered":m.metered,"credit_balance":m.balance,"total_allocated":5,"credit_transaction":receipt});
        if m.missing_policy {
            payload.as_object_mut().unwrap().remove("metered");
        }
        let signer = SigningKey::from_bytes(&[9; 32]);
        let pubkey = signer.verifying_key().to_bytes();
        let encoded = serde_json::to_string(&payload).unwrap();
        let envelope = json!({"payload":payload,"payload_json":encoded,"signature":B64.encode(signer.sign(encoded.as_bytes()).to_bytes()),"algorithm":"Ed25519","key_id":format!("ed25519_{}",&format!("{:x}",Sha256::digest(pubkey))[..16])});
        let mut data = json!({"success":status==200,"activation_token":"act_SYNTHETIC_FIXTURE","session_token":seat,"license":envelope});
        if let Some(code) = code {
            data["error"] = json!({"code":code});
            data["balance"] = json!(m.balance);
        } else if r.path.ends_with("/consume") {
            data["transaction_id"] = receipt["id"].clone();
            data["balance"] =
                json!(receipt["balance_after"].as_u64().unwrap() + u64::from(m.tamper));
            data["consumed"] = body["amount"].clone();
            data["replayed"] = receipt["replayed"].clone();
        }
        reply(data, status)
    }
}
struct Fixture {
    client: LicenseClient,
    transport: Arc<Mock>,
    now: Arc<AtomicI64>,
    hardware: Arc<Mutex<(String, String)>>,
    _directory: tempfile::TempDir,
}
fn fixture(floating: bool, metered: bool) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let now = Arc::new(AtomicI64::new(NOW));
    let transport = Arc::new(Mock {
        mode: Mutex::new(Mode {
            floating,
            metered,
            ..Mode::default()
        }),
        now: now.clone(),
    });
    let hardware_state = Arc::new(Mutex::new((String::from("nic-1"), String::from("board-1"))));
    let fp = hardware_state.clone();
    let clock = now.clone();
    let signer = SigningKey::from_bytes(&[9; 32]);
    let raw = signer.verifying_key().to_bytes();
    let key=VerificationKey::from_json(&serde_json::to_vec(&json!({"algorithm":"Ed25519","encoding":"base64url","public_key":B64.encode(raw),"key_id":format!("ed25519_{}",&format!("{:x}",Sha256::digest(raw))[..16])})).unwrap()).unwrap();
    let client = LicenseClient::builder(APP)
        .environment(Environment::Sandbox)
        .storage_dir(directory.path())
        .transport(transport.clone())
        .verification_key(key)
        .fingerprint_provider(move |_| {
            let h = fp.lock().unwrap();
            Ok(hardware(&h.0, &h.1))
        })
        .clock(move || DateTime::<Utc>::from_timestamp(clock.load(Ordering::SeqCst), 0).unwrap())
        .build()
        .unwrap();
    Fixture {
        client,
        transport,
        now,
        hardware: hardware_state,
        _directory: directory,
    }
}
#[test]
fn required_fingerprint_and_fuzzy_hardware_binding() {
    let f = fixture(false, false);
    assert!(f.client.activate("FIXTURE", None).valid());
    f.hardware.lock().unwrap().0 = "nic-2".into();
    assert!(f.client.check().valid());
    f.hardware.lock().unwrap().1 = "board-2".into();
    assert_eq!(
        f.client.validate().error_code(),
        Some(ErrorCode::FingerprintMismatch)
    );
    assert!(HardwareFingerprint::from_components(
        APP,
        &BTreeMap::from([("cpu".into(), "cpu-only".into())]),
        None
    )
    .is_err());
}
#[test]
fn automatic_checkout_heartbeat_recovery_and_explicit_checkin() {
    let f = fixture(true, false);
    let held = f.client.activate("FIXTURE", None);
    assert!(held.valid());
    assert!(f.client.checkout().unwrap().valid());
    f.transport.mode.lock().unwrap().missing = true;
    assert!(f.client.heartbeat().valid());
    assert!(f.client.checkin().deactivated());
    assert!(!held.feature("export"));
    assert!(!f.transport.mode.lock().unwrap().held);
}
#[test]
fn signed_lease_expires_before_offline_grace() {
    let f = fixture(true, false);
    let held = f.client.activate("FIXTURE", None);
    f.transport.mode.lock().unwrap().offline = true;
    f.now.store(NOW + 599, Ordering::SeqCst);
    assert!(f.client.validate().valid());
    f.now.store(NOW + 600, Ordering::SeqCst);
    assert!(!held.valid());
    assert_eq!(
        f.client.validate().error_code(),
        Some(ErrorCode::SessionExpired)
    );
}
#[test]
fn full_seat_error_and_missing_contract_fail_closed() {
    let f = fixture(true, false);
    f.transport.mode.lock().unwrap().full = true;
    assert_eq!(
        f.client.activate("FIXTURE", None).error_code(),
        Some(ErrorCode::AllSeatsInUse)
    );
    assert_eq!(
        f.client.checkout().unwrap_err().code(),
        ErrorCode::AllSeatsInUse
    );
    let g = fixture(false, false);
    g.transport.mode.lock().unwrap().missing_policy = true;
    assert!(!g.client.activate("FIXTURE", None).valid());
}
#[test]
fn signed_credits_last_unit_replay_and_zero_balance() {
    let f = fixture(false, true);
    assert!(f.client.activate("FIXTURE", None).valid());
    let first = f.client.consume(2, Some("job-fixture-1"), None).unwrap();
    assert_eq!(first.balance, 3);
    assert_eq!(
        f.client
            .consume(3, Some("job-fixture-2"), None)
            .unwrap()
            .balance,
        0
    );
    let replay = f.client.consume(2, Some("job-fixture-1"), None).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.transaction_id, first.transaction_id);
    assert_eq!(replay.balance, 3);
    assert_eq!(f.client.get_balance().unwrap(), 0);
    assert_eq!(
        f.client
            .consume(1, Some("job-fixture-3"), None)
            .unwrap_err(),
        CreditError::InsufficientCredits { balance: 0 }
    );
}
#[test]
fn zero_activation_survives_topup_online_only_and_no_automatic_retry() {
    let f = fixture(false, true);
    f.transport.mode.lock().unwrap().balance = 0;
    assert_eq!(
        f.client.activate("FIXTURE", None).error_code(),
        Some(ErrorCode::InsufficientCredits)
    );
    assert!(f.client.activated().unwrap());
    f.transport.mode.lock().unwrap().balance = 5;
    assert_eq!(f.client.get_balance().unwrap(), 5);
    f.transport.mode.lock().unwrap().lost = true;
    assert_eq!(
        f.client
            .consume(2, Some("durable-fixture"), None)
            .unwrap_err()
            .code(),
        ErrorCode::Network
    );
    assert!(
        f.client
            .consume(2, Some("durable-fixture"), None)
            .unwrap()
            .replayed
    );
    f.transport.mode.lock().unwrap().offline = true;
    assert_eq!(
        f.client.check().error_code(),
        Some(ErrorCode::MeteredOnlineRequired)
    );
    assert!(f.client.get_balance().is_err());
}
#[test]
fn idempotency_conflict_unsigned_tampering_and_tenant_binding() {
    let f = fixture(false, true);
    assert!(f.client.activate("FIXTURE", None).valid());
    f.client.consume(1, Some("conflict-fixture"), None).unwrap();
    assert_eq!(
        f.client
            .consume(2, Some("conflict-fixture"), None)
            .unwrap_err()
            .code(),
        ErrorCode::IdempotencyConflict
    );
    f.transport.mode.lock().unwrap().tamper = true;
    assert!(f.client.consume(1, Some("tamper-fixture"), None).is_err());
    f.transport.mode.lock().unwrap().tamper = false;
    f.transport.mode.lock().unwrap().wrong_tenant = true;
    assert!(f.client.consume(1, Some("tenant-fixture"), None).is_err());
    assert!(f.client.consume(0, None, None).is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_credit_and_seat_wrappers_use_the_same_contract() {
    let f = fixture(false, true);
    let builder = LicenseClient::builder(APP)
        .storage_dir(f._directory.path())
        .transport(f.transport.clone())
        .clock(|| DateTime::<Utc>::from_timestamp(NOW, 0).unwrap())
        .fingerprint_provider(|_| Ok(hardware("nic-1", "board-1")));
    let client = kdpika_licensing::AsyncLicenseClient::build(builder)
        .await
        .unwrap();
    assert!(client.activate("FIXTURE", None).await.unwrap().valid());
    assert_eq!(
        client
            .consume(1, Some("async-job-fixture".into()), None)
            .await
            .unwrap()
            .balance,
        4
    );
    assert_eq!(client.get_balance().await.unwrap(), 4);
    assert!(client.close().await.unwrap().deactivated());
}
