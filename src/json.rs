//! API v1 canonical JSON is PHP's format, NOT RFC 8785. Preserve numeric lexemes.
use crate::error::{Error, ErrorCode, Result};
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::{value::RawValue, Value};
use std::{collections::BTreeMap, fmt};
pub(crate) const MAX_BYTES: usize = 1_048_576;
pub(crate) struct Object(pub BTreeMap<String, Box<RawValue>>);
impl<'de> Deserialize<'de> for Object {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Unique;
        impl<'de> Visitor<'de> for Unique {
            type Value = Object;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("unique JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Object, M::Error> {
                let mut result = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    if result.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate property"));
                    }
                }
                Ok(Object(result))
            }
        }
        deserializer.deserialize_map(Unique)
    }
}
fn invalid() -> Error {
    Error(ErrorCode::InvalidResponse)
}
fn quote(text: &str) -> Result<String> {
    Ok(serde_json::to_string(text)
        .map_err(|_| invalid())?
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029"))
}
pub(crate) fn canonical(raw: &RawValue, depth: usize) -> Result<String> {
    if depth > 64 {
        return Err(invalid());
    }
    let text = raw.get();
    match text.as_bytes().first() {
        Some(b'{') => {
            let Object(map) = serde_json::from_str(text).map_err(|_| invalid())?;
            let mut parts = Vec::new();
            for (k, v) in map {
                parts.push(format!("{}:{}", quote(&k)?, canonical(&v, depth + 1)?));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        Some(b'[') => {
            let list: Vec<Box<RawValue>> = serde_json::from_str(text).map_err(|_| invalid())?;
            let parts: Result<Vec<_>> = list.iter().map(|v| canonical(v, depth + 1)).collect();
            Ok(format!("[{}]", parts?.join(",")))
        }
        Some(b'"') => {
            let value: String = serde_json::from_str(text).map_err(|_| invalid())?;
            quote(&value)
        }
        _ => {
            if text != "true"
                && text != "false"
                && text != "null"
                && !text.parse::<f64>().is_ok_and(|n| n.is_finite())
            {
                return Err(invalid());
            }
            Ok(text.to_owned())
        }
    }
}
pub(crate) fn parse(raw: &[u8]) -> Result<Object> {
    if raw.len() > MAX_BYTES {
        return Err(invalid());
    }
    let value: Box<RawValue> = serde_json::from_slice(raw).map_err(|_| invalid())?;
    canonical(&value, 0)?;
    serde_json::from_str(value.get()).map_err(|_| invalid())
}
pub(crate) fn value(raw: &[u8]) -> Result<Value> {
    parse(raw)?;
    serde_json::from_slice(raw).map_err(|_| invalid())
}
pub(crate) fn field(map: &Object, key: &str) -> Result<String> {
    serde_json::from_str(map.0.get(key).ok_or_else(invalid)?.get()).map_err(|_| invalid())
}
