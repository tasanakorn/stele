use serde::de::{self, Deserialize, DeserializeOwned, Deserializer};
use serde_json::Value;

/// Deserialize a `Vec<T>` that may arrive as either a JSON array or a JSON-encoded string.
///
/// Claude Code sometimes sends array parameters as `"[{...}]"` (a string containing JSON)
/// instead of `[{...}]` (an actual JSON array). This function accepts both forms.
pub fn string_or_vec<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(_) => serde_json::from_value(value).map_err(de::Error::custom),
        Value::String(ref s) => serde_json::from_str(s).map_err(de::Error::custom),
        other => Err(de::Error::custom(format!(
            "expected array or string-encoded array, got {}",
            value_type_name(&other)
        ))),
    }
}

/// Deserialize an `Option<Vec<T>>` that may arrive as a JSON array, a JSON-encoded string,
/// or null/missing.
pub fn string_or_vec_opt<'de, T, D>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::Array(_) => serde_json::from_value(value)
            .map(Some)
            .map_err(de::Error::custom),
        Value::String(ref s) => serde_json::from_str(s)
            .map(Some)
            .map_err(de::Error::custom),
        other => Err(de::Error::custom(format!(
            "expected array, string-encoded array, or null, got {}",
            value_type_name(&other)
        ))),
    }
}

/// Deserialize a `Vec<String>` from either a single string or an array of strings.
///
/// Unlike `string_or_vec`, a bare string `"foo"` becomes `vec!["foo"]` directly —
/// it is NOT parsed as a JSON-encoded array.
pub fn string_or_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(arr) => arr
            .into_iter()
            .map(|v| match v {
                Value::String(s) => Ok(s),
                other => Err(de::Error::custom(format!(
                    "expected string in array, got {}",
                    value_type_name(&other)
                ))),
            })
            .collect(),
        Value::String(s) => Ok(vec![s]),
        other => Err(de::Error::custom(format!(
            "expected string or array of strings, got {}",
            value_type_name(&other)
        ))),
    }
}

/// Deserialize an `Option<Vec<String>>` from null/missing, a single string, or an array of strings.
pub fn string_or_string_vec_opt<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(None),
        Value::Array(arr) => arr
            .into_iter()
            .map(|v| match v {
                Value::String(s) => Ok(s),
                other => Err(de::Error::custom(format!(
                    "expected string in array, got {}",
                    value_type_name(&other)
                ))),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Value::String(s) => Ok(Some(vec![s])),
        other => Err(de::Error::custom(format!(
            "expected string, array of strings, or null, got {}",
            value_type_name(&other)
        ))),
    }
}

fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
