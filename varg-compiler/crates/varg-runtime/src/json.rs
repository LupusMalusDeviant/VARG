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

/// Parse a JSON document, reporting what was wrong with it if it will not parse.
///
/// This used to lower to `from_str(..).unwrap_or(Value::Null)`, so a malformed document became
/// an empty one and every later read answered as though the keys were simply missing. serde
/// already knows the line and column; handing that back costs nothing and is the difference
/// between "this key is absent" and "this is not JSON".
pub fn __varg_json_parse(s: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(s).map_err(|e| format!("invalid JSON: {}", e))
}

/// Render a value the way `json_get` reports it: a string is its own text (no quotes), any
/// other kind is its JSON text. Rendering matters because the old accessor filtered with
/// `as_str()`, so a number, a bool or a nested object all came back as `""` — the same answer
/// as a missing key.
fn render(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// The value at `path` as text, or `None` if there is none there.
///
/// `None` means exactly one thing: nothing to read. That covers an absent key and an explicit
/// JSON `null` (which *is* the absence of a value), and nothing else — a present number, bool,
/// array or object renders as its text rather than vanishing.
pub fn __varg_json_get<J: AsJson + ?Sized>(j: &J, path: &str) -> Option<String> {
    let v = j.as_json();
    match lookup(&v, path) {
        None | Some(Value::Null) => None,
        Some(found) => Some(render(found)),
    }
}

/// The integer at `path`, or `None` if there is no integer there.
///
/// Strict on purpose: a string `"42"` is not an integer and answers `None`. Use `json_get` and
/// `parse_int` when the document really does carry numbers as text.
pub fn __varg_json_get_int<J: AsJson + ?Sized>(j: &J, path: &str) -> Option<i64> {
    let v = j.as_json();
    lookup(&v, path).and_then(|x| x.as_i64())
}

/// The boolean at `path`, or `None` if there is no boolean there.
pub fn __varg_json_get_bool<J: AsJson + ?Sized>(j: &J, path: &str) -> Option<bool> {
    let v = j.as_json();
    lookup(&v, path).and_then(|x| x.as_bool())
}

/// The array at `path` as text elements, or `None` if there is no array there.
///
/// Elements render like `json_get`, so a `[1, 2]` yields `["1", "2"]`. The old version filtered
/// with `as_str()` and dropped every non-string element, which turned a numeric array into an
/// empty one without saying so.
pub fn __varg_json_get_array<J: AsJson + ?Sized>(j: &J, path: &str) -> Option<Vec<String>> {
    let v = j.as_json();
    lookup(&v, path)
        .and_then(|x| x.as_array())
        .map(|a| a.iter().map(render).collect())
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
        assert_eq!(__varg_json_get(&parsed(), "name").as_deref(), Some("varg"));
        assert_eq!(__varg_json_get(&DOC.to_string(), "name").as_deref(), Some("varg"));
        // Pointer paths reach nested values in both shapes.
        assert_eq!(__varg_json_get(&parsed(), "/main/temp").as_deref(), Some("warm"));
        assert_eq!(__varg_json_get(&DOC.to_string(), "/main/temp").as_deref(), Some("warm"));
    }

    #[test]
    fn typed_getters_work_on_both_shapes() {
        assert_eq!(__varg_json_get_int(&parsed(), "/n"), Some(42));
        assert_eq!(__varg_json_get_int(&DOC.to_string(), "/n"), Some(42));
        assert_eq!(__varg_json_get_bool(&parsed(), "ok"), Some(true));
        assert_eq!(__varg_json_get_bool(&DOC.to_string(), "ok"), Some(true));
        assert_eq!(__varg_json_get_array(&parsed(), "tags"), Some(vec!["a".into(), "b".into()]));
        assert_eq!(__varg_json_get_array(&DOC.to_string(), "tags"), Some(vec!["a".into(), "b".into()]));
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
    fn absence_is_none_and_is_the_only_none() {
        // Nothing there: the one situation that answers None.
        assert_eq!(__varg_json_get(&DOC.to_string(), "nope"), None);
        assert_eq!(__varg_json_get_int(&DOC.to_string(), "nope"), None);
        assert_eq!(__varg_json_get_bool(&DOC.to_string(), "nope"), None);
        assert_eq!(__varg_json_get_array(&DOC.to_string(), "nope"), None);

        // Present but not a string used to answer `""` — the same as absent. Now each renders.
        assert_eq!(__varg_json_get(&DOC.to_string(), "n").as_deref(), Some("42"));
        assert_eq!(__varg_json_get(&DOC.to_string(), "ok").as_deref(), Some("true"));
        assert_eq!(__varg_json_get(&DOC.to_string(), "tags").as_deref(), Some(r#"["a","b"]"#));
        assert_eq!(
            __varg_json_get(&DOC.to_string(), "main").as_deref(),
            Some(r#"{"temp":"warm"}"#),
            "a nested object is readable instead of vanishing"
        );

        // A present empty string is a value, and stays distinguishable from absence.
        let empty = r#"{"s":""}"#.to_string();
        assert_eq!(__varg_json_get(&empty, "s").as_deref(), Some(""));
        assert_eq!(__varg_json_get(&empty, "t"), None);

        // An explicit JSON null is the absence of a value, so it reads as None.
        assert_eq!(__varg_json_get(&r#"{"a":null}"#.to_string(), "a"), None);

        // The typed getters stay strict: a value of the wrong kind is not that kind.
        assert_eq!(__varg_json_get_int(&DOC.to_string(), "name"), None, "\"varg\" is no int");
        assert_eq!(__varg_json_get_bool(&DOC.to_string(), "n"), None, "42 is no bool");
        assert_eq!(__varg_json_get_array(&DOC.to_string(), "name"), None);

        // Non-string array elements render instead of being dropped.
        assert_eq!(
            __varg_json_get_array(&r#"{"xs":[1,2]}"#.to_string(), "xs"),
            Some(vec!["1".to_string(), "2".to_string()]),
            "a numeric array used to come back empty"
        );

        // Unparseable input has no values in it either.
        let junk = "not json at all".to_string();
        assert_eq!(__varg_json_get(&junk, "name"), None);
        assert!(__varg_json_keys(&junk).is_empty());
    }
}
