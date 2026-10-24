use crate::{
    error::{Error, ErrorCode, Result},
    json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Independently trusted Ed25519 key document. Fetch/pin through your application publisher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationKey {
    key_id: String,
    public_key: String,
    algorithm: String,
    encoding: String,
}
impl VerificationKey {
    /// Parse and check algorithm, canonical base64url and key-derived identifier.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let value = json::value(bytes)?;
        let key: Self =
            serde_json::from_value(value).map_err(|_| Error(ErrorCode::SignatureInvalid))?;
        key.validate()?;
        Ok(key)
    }
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
    pub(crate) fn validate(&self) -> Result<[u8; 32]> {
        let raw = decode(&self.public_key, 32)?;
        let id = format!("ed25519_{}", &hash(&raw)[..16]);
        if self.algorithm != "Ed25519" || self.encoding != "base64url" || self.key_id != id {
            return Err(Error(ErrorCode::SignatureInvalid));
        }
        raw.try_into()
            .map_err(|_| Error(ErrorCode::SignatureInvalid))
    }
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn decode(text: &str, size: usize) -> Result<Vec<u8>> {
    let raw = URL_SAFE_NO_PAD
        .decode(text)
        .map_err(|_| Error(ErrorCode::SignatureInvalid))?;
    if raw.len() != size || URL_SAFE_NO_PAD.encode(&raw) != text {
        return Err(Error(ErrorCode::SignatureInvalid));
    }
    Ok(raw)
}
pub(crate) fn verify(key: &VerificationKey, envelope: &str) -> Result<Value> {
    let public = key.validate()?;
    let env = json::parse(envelope.as_bytes()).map_err(|_| Error(ErrorCode::SignatureInvalid))?;
    if json::field(&env, "algorithm")? != "Ed25519" || json::field(&env, "key_id")? != key.key_id {
        return Err(Error(ErrorCode::SignatureInvalid));
    }
    let payload = env
        .0
        .get("payload")
        .ok_or(Error(ErrorCode::SignatureInvalid))?;
    if !payload.get().starts_with('{') {
        return Err(Error(ErrorCode::SignatureInvalid));
    }
    let bytes = json::canonical(payload, 0)?;
    if env.0.contains_key("payload_json") && json::field(&env, "payload_json")? != bytes {
        return Err(Error(ErrorCode::SignatureInvalid));
    }
    let signature = Signature::from_slice(&decode(&json::field(&env, "signature")?, 64)?)
        .map_err(|_| Error(ErrorCode::SignatureInvalid))?;
    VerifyingKey::from_bytes(&public)
        .map_err(|_| Error(ErrorCode::SignatureInvalid))?
        .verify_strict(bytes.as_bytes(), &signature)
        .map_err(|_| Error(ErrorCode::SignatureInvalid))?;
    serde_json::from_str(payload.get()).map_err(|_| Error(ErrorCode::SignatureInvalid))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_published_php_signature_vectors() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
        let index: Value =
            serde_json::from_slice(&std::fs::read(root.join("index.json")).unwrap()).unwrap();
        for row in index["vectors"].as_array().unwrap() {
            let key = VerificationKey::from_json(
                &std::fs::read(root.join(row["key"].as_str().unwrap())).unwrap(),
            );
            let envelope = std::fs::read(root.join(row["envelope"].as_str().unwrap())).unwrap();
            let result = key
                .and_then(|key| {
                    std::str::from_utf8(&envelope)
                        .map_err(|_| Error(ErrorCode::SignatureInvalid))
                        .and_then(|s| verify(&key, s))
                })
                .is_ok();
            assert_eq!(
                result,
                row["signature_valid"].as_bool().unwrap(),
                "vector {}",
                row["name"]
            );
        }
    }
    #[test]
    fn hostile_json_is_rejected() {
        for raw in [
            b"{\"a\":1,\"a\":2}".as_slice(),
            b"{\"a\":1e999}",
            b"{\"a\":NaN}",
            b"{\"a\":\"\\ud800\"}",
            b"{}x",
            b"{\"a\":\"\xff\"}",
        ] {
            assert!(json::parse(raw).is_err());
        }
        assert!(json::parse(&vec![b' '; 1_048_577]).is_err());
    }
    #[test]
    fn numeric_tokens_and_unicode_survive_cache_roundtrip() {
        for token in [
            "19.0",
            "1.0e-7",
            "1.0e+20",
            "-0.0",
            "9223372036854775807",
            "9007199254740993",
        ] {
            let raw = format!("{{\"n\":{token},\"s\":\"a\\u2028b\\u2029c\"}}");
            let first = json::canonical(&serde_json::value::RawValue::from_string(raw).unwrap(), 0)
                .unwrap();
            assert!(first.contains(token));
            assert!(first.contains("\\u2028"));
            assert_eq!(
                first,
                json::canonical(
                    &serde_json::value::RawValue::from_string(first.clone()).unwrap(),
                    0
                )
                .unwrap()
            );
        }
    }
}
