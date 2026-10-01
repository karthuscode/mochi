//! Strict untrusted JSON decoding, without diagnostic payloads.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;
struct Seed(usize);
struct StrictVisitor(usize);
impl<'de> DeserializeSeed<'de> for Seed {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        if self.0 > 32 {
            return Err(de::Error::custom("bounds"));
        }
        d.deserialize_any(StrictVisitor(self.0))
    }
}
impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON")
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("number"))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(v) = a.next_element_seed(Seed(self.0 + 1))? {
            if values.len() >= 2048 {
                return Err(de::Error::custom("bounds"));
            }
            values.push(v);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = a.next_key::<String>()? {
            if values.len() >= 256 || values.contains_key(&key) {
                return Err(de::Error::custom("duplicate or bounds"));
            }
            values.insert(key, a.next_value_seed(Seed(self.0 + 1))?);
        }
        Ok(Value::Object(values))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidJson;
pub fn parse_bounded_json(bytes: &[u8], max_bytes: usize) -> Result<Value, InvalidJson> {
    if bytes.is_empty() || bytes.len() > max_bytes {
        return Err(InvalidJson);
    }
    let mut d = serde_json::Deserializer::from_slice(bytes);
    let v = Seed(0).deserialize(&mut d).map_err(|_| InvalidJson)?;
    d.end().map_err(|_| InvalidJson)?;
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_nested_keys_and_bounds_are_rejected() {
        assert!(parse_bounded_json(br#"{"input":{"path":"safe","path":"secret"}}"#, 1024).is_err());
        assert!(parse_bounded_json(b"{} trailing", 1024).is_err());
        let deep = format!("{}0{}", "[".repeat(40), "]".repeat(40));
        assert!(parse_bounded_json(deep.as_bytes(), 1024).is_err());
        assert!(parse_bounded_json(b"{}", 1).is_err());
        assert!(parse_bounded_json(br#"{"input":{"path":"safe"}}"#, 1024).is_ok());
    }
}
