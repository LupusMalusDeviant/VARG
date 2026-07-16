// JSON accessors that accept either shape of input.
//
// The json builtins used to disagree with each other: `json_get*` required an already-parsed value
// (from `json_parse`), while `json_keys`/`json_values`/`json_has` required a raw JSON *string*. So
// whichever you had, half the family rejected it — and a JSON string coming straight out of another
// builtin could not be read with `json_get` at all without a `json_parse` hop.
//
// Everything here takes `impl AsJson`, so both a parsed value and a raw string work everywhere.

use serde_json::Value;
use std::borrow::Cow;

/// JSON input: either an already-parsed `Value` or a raw JSON string.
pub trait AsJson {
    fn as_json(&self) -> Cow<'_, Value>;
}

impl AsJson for Value {
    fn as_json(&self) -> Cow<'_, Value> {
        Cow::Borrowed(self)
    }
}

impl AsJson for String {
    fn as_json(&self) -> Cow<'_, Value> {
        Cow::Owned(serde_json::from_str(self).unwrap_or(Value::Null))
    }
}

impl AsJson for str {
    fn as_json(&self) -> Cow<'_, Value> {
        Cow::Owned(serde_json::from_str(self).unwrap_or(Value::Null))
    }
}

/// A leading `/` selects a JSON pointer (nested, e.g. "/a/b"); otherwise a single object key.
fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    if path.starts_with('/') {
        v.pointer(path)
    } else {
        v.get(path)
    }
}

pub fn __varg_json_get<J: AsJson + ?Sized>(j: &J, path: &str) -> String {
    let v = j.as_json();
    lookup(&v, path)
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

pub fn __varg_json_get_int<J: AsJson + ?Sized>(j: &J, path: &str) -> i64 {
    let v = j.as_json();
    lookup(&v, path).and_then(|x| x.as_i64()).unwrap_or(0)
}

pub fn __varg_json_get_bool<J: AsJson + ?Sized>(j: &J, path: &str) -> bool {
    let v = j.as_json();
    lookup(&v, path).and_then(|x| x.as_bool()).unwrap_or(false)
}

pub fn __varg_json_get_array<J: AsJson + ?Sized>(j: &J, path: &str) -> Vec<String> {
    let v = j.as_json();
    lookup(&v, path)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

pub fn __varg_json_has<J: AsJson + ?Sized>(j: &J, path: &str) -> bool {
    let v = j.as_json();
    lookup(&v, path).is_some()
}

pub fn __varg_json_keys<J: AsJson + ?Sized>(j: &J) -> Vec<String> {
    let v = j.as_json();
    v.as_object()
        .map(|o| o.keys().map(|k| k.to_string()).collect())
        .unwrap_or_default()
}

pub fn __varg_json_values<J: AsJson + ?Sized>(j: &J) -> Vec<String> {
    let v = j.as_json();
    v.as_object()
        .map(|o| o.values().map(|x| serde_json::to_string(x).unwrap_or_default()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"{"name":"varg","n":42,"ok":true,"tags":["a","b"],"main":{"temp":"warm"}}"#;

    fn parsed() -> Value {
        serde_json::from_str(DOC).unwrap()
    }

    // The point of the trait: the same call works on a parsed value and on a raw JSON string.

    #[test]
    fn get_works_on_parsed_value_and_on_raw_string() {
        assert_eq!(__varg_json_get(&parsed(), "name"), "varg");
        assert_eq!(__varg_json_get(&DOC.to_string(), "name"), "varg");
        // Pointer paths reach nested values in both shapes.
        assert_eq!(__varg_json_get(&parsed(), "/main/temp"), "warm");
        assert_eq!(__varg_json_get(&DOC.to_string(), "/main/temp"), "warm");
    }

    #[test]
    fn typed_getters_work_on_both_shapes() {
        assert_eq!(__varg_json_get_int(&parsed(), "/n"), 42);
        assert_eq!(__varg_json_get_int(&DOC.to_string(), "/n"), 42);
        assert!(__varg_json_get_bool(&parsed(), "ok"));
        assert!(__varg_json_get_bool(&DOC.to_string(), "ok"));
        assert_eq!(__varg_json_get_array(&parsed(), "tags"), vec!["a", "b"]);
        assert_eq!(__varg_json_get_array(&DOC.to_string(), "tags"), vec!["a", "b"]);
    }

    #[test]
    fn has_and_keys_work_on_both_shapes() {
        assert!(__varg_json_has(&parsed(), "name"));
        assert!(__varg_json_has(&DOC.to_string(), "name"));
        assert!(!__varg_json_has(&DOC.to_string(), "missing"));
        assert!(__varg_json_has(&DOC.to_string(), "/main/temp"), "pointer paths too");

        let mut keys = __varg_json_keys(&DOC.to_string());
        keys.sort();
        assert_eq!(keys, vec!["main", "n", "name", "ok", "tags"]);
        assert_eq!(__varg_json_keys(&parsed()).len(), 5);
        assert_eq!(__varg_json_values(&parsed()).len(), 5);
    }

    #[test]
    fn missing_paths_and_bad_json_degrade_quietly() {
        assert_eq!(__varg_json_get(&DOC.to_string(), "nope"), "");
        assert_eq!(__varg_json_get_int(&DOC.to_string(), "nope"), 0);
        assert!(!__varg_json_get_bool(&DOC.to_string(), "nope"));
        assert!(__varg_json_get_array(&DOC.to_string(), "nope").is_empty());
        // Unparsable input becomes Null rather than panicking.
        let junk = "not json".to_string();
        assert_eq!(__varg_json_get(&junk, "name"), "");
        assert!(__varg_json_keys(&junk).is_empty());
    }
}
