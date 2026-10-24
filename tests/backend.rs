//! Explicitly opted-in local real-PHP/MySQL acceptance. Never uses production keys.
use kdpika_licensing::{Environment, ErrorCode, LicenseClient, Source};
use serde_json::Value;
#[test]
#[ignore = "requires isolated loopback PHP fixture; never run against a customer's license"]
fn actual_php_mysql_lifecycle() {
    let base = std::env::var("KDP_RUST_TEST_ORIGIN").unwrap();
    assert!(base.starts_with("http://127.0.0.1:"));
    let path = std::env::var("KDP_RUST_TEST_FIXTURE").unwrap();
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(fixture["database"]
        .as_str()
        .unwrap()
        .starts_with("lmp_rust_sdk_"));
    let dir = tempfile::tempdir().unwrap();
    let client = LicenseClient::builder(fixture["app_id"].as_str().unwrap())
        .environment(Environment::Sandbox)
        .storage_dir(dir.path())
        .base_url(&base)
        .allow_loopback_http(true)
        .build()
        .unwrap();
    let key = fixture["license_key"].as_str().unwrap();
    assert_eq!(
        client
            .activate(key, Some("wrong@example.test"))
            .error_code(),
        Some(ErrorCode::InvalidLicense)
    );
    let result = client.activate(key, Some(fixture["email"].as_str().unwrap()));
    assert!(result.valid(), "safe result: {result:?}");
    assert!(result.signature_valid() && result.feature("pdf_export"));
    assert_eq!(result.limit("seat_count"), Some(3));
    assert_eq!(
        result.feature_value("price_limit").unwrap().to_string(),
        "19.0"
    );
    assert_eq!(result.feature_value("unicode").unwrap(), "café العربية 😀");
    assert_eq!(client.check().source(), Source::Cache);
    assert!(client.validate().valid());
    let second = LicenseClient::builder(fixture["app_id"].as_str().unwrap())
        .storage_dir(dir.path())
        .base_url(&base)
        .allow_loopback_http(true)
        .build()
        .unwrap();
    assert_eq!(client.installation_id(), second.installation_id());
    assert!(second.check().valid());
    let released = second.deactivate();
    assert!(released.deactivated() && !released.valid());
    assert_eq!(client.check().error_code(), Some(ErrorCode::NotActivated));
}
