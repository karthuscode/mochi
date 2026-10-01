use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::Path;

pub const CAPTURE_EVENTS: [&str; 12] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
    "Interrupt",
    "PreCompact",
    "PostCompact",
    "SubagentStart",
    "SubagentStop",
];
pub(crate) const MAX_CONFIG_BYTES: u64 = 256 * 1024;

/// No config content or underlying filesystem errors escape this boundary.
pub(crate) fn read_config(path: &Path) -> Result<Option<Vec<u8>>, ()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_CONFIG_BYTES
    {
        return Err(());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| ())?;
    if !file.metadata().map_err(|_| ())?.is_file() {
        return Err(());
    }
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(());
    }
    Ok(Some(bytes))
}

pub(crate) fn parse_config(bytes: &[u8]) -> Result<Value, ()> {
    use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
    use std::fmt;
    struct Seed(usize);
    struct StrictVisitor(usize);
    impl<'de> DeserializeSeed<'de> for Seed {
        type Value = Value;
        fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
            if self.0 > 16 {
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
                if values.len() >= 256 {
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
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = Seed(0).deserialize(&mut deserializer).map_err(|_| ())?;
    deserializer.end().map_err(|_| ())?;
    if !value.is_object() {
        return Err(());
    }
    if let Some(hooks) = value.get("hooks") {
        let events = hooks.as_object().ok_or(())?;
        for groups in events.values() {
            for group in groups.as_array().ok_or(())? {
                let group = group.as_object().ok_or(())?;
                if let Some(matcher) = group.get("matcher") {
                    if !matcher.is_string() {
                        return Err(());
                    }
                }
                for handler in group.get("hooks").and_then(Value::as_array).ok_or(())? {
                    if !handler.is_object() || !handler.get("type").is_some_and(Value::is_string) {
                        return Err(());
                    }
                }
            }
        }
    }
    Ok(value)
}

pub(crate) fn shell_quote(value: &str) -> Result<String, ()> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(());
    }
    Ok(format!("'{}'", value.replace('\'', "'\\''")))
}

/// Parse literal arguments only. Substitutions/operators never establish readiness.
pub(crate) fn literal_words(command: &str) -> Option<Vec<String>> {
    if command.len() > 16 * 1024 || command.chars().any(char::is_control) {
        return None;
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), c) => word.push(c),
            (Some('"'), '$' | '`' | '\\') => return None,
            (Some('"'), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                started = true;
            }
            (None, '\\') => {
                word.push(chars.next()?);
                started = true;
            }
            (None, ' ') => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            (None, c) if c.is_alphanumeric() || "/._-=:".contains(c) => {
                word.push(c);
                started = true;
            }
            _ => return None,
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        words.push(word);
    }
    (words.len() <= 32).then_some(words)
}

pub(crate) fn valid_capture_command(command: &str, helper: &Path) -> bool {
    let Some(words) = literal_words(command) else {
        return false;
    };
    if words.first().map(String::as_str) != helper.to_str()
        || words.get(1).map(String::as_str) != Some("capture")
    {
        return false;
    }
    let mut fields = std::collections::BTreeMap::new();
    let (pairs, remainder) = words[2..].as_chunks::<2>();
    if !remainder.is_empty() {
        return false;
    }
    for pair in pairs {
        if fields.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return false;
        }
    }
    (fields.len() == 4 || fields.len() == 5)
        && fields
            .get("--project-id")
            .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
        && fields
            .get("--approved-root")
            .is_some_and(|s| Path::new(s).is_absolute())
        && fields.get("--client-surface") == Some(&"cli")
        && fields
            .get("--policy-revision")
            .is_some_and(|s| s.parse::<u64>().is_ok_and(|n| n > 0))
        && (!fields.contains_key("--spool-root")
            || fields
                .get("--spool-root")
                .is_some_and(|s| Path::new(s).is_absolute()))
}
