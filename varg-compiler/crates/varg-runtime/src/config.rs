//! Wave 29: Config cascade + platform directories
//!
//! Provides:
//! - Platform-specific directory lookups (home, config, data, cache)
//! - JSON config cascade: load a list of JSON files and deep-merge them
//!   (later files override earlier ones, matching XDG / dotfile conventions).

use serde_json::Value;
use std::path::PathBuf;

fn pathbuf_to_string(p: Option<PathBuf>) -> String {
    p.map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

pub fn __varg_home_dir() -> String {
    pathbuf_to_string(dirs::home_dir())
}

pub fn __varg_config_dir() -> String {
    pathbuf_to_string(dirs::config_dir())
}

pub fn __varg_data_dir() -> String {
    pathbuf_to_string(dirs::data_dir())
}

pub fn __varg_cache_dir() -> String {
    pathbuf_to_string(dirs::cache_dir())
}

/// Deep-merge two JSON values. Values in `b` override values in `a`.
/// Objects are merged key-by-key (recursive). Arrays and scalars are replaced.
fn merge_json(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a_map), Value::Object(b_map)) => {
            for (k, v) in b_map {
                match a_map.get_mut(&k) {
                    Some(existing) => merge_json(existing, v),
                    None => {
                        a_map.insert(k, v);
                    }
                }
            }
        }
        (a_slot, b_val) => {
            *a_slot = b_val;
        }
    }
}

/// Load a series of JSON files and merge them. Missing files are skipped
/// silently (cascade semantics — user config is optional).
/// Returns the merged JSON as a string, or an error if a present file fails
/// to parse.
pub fn __varg_config_load_cascade(paths: &[String]) -> Result<String, String> {
    let mut merged = Value::Object(serde_json::Map::new());
    for path in paths {
        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue, // missing file — skip (cascade)
        };
        let parsed: Value = serde_json::from_str(&contents)
            .map_err(|e| format!("parse error in {}: {}", path, e))?;
        merge_json(&mut merged, parsed);
    }
    serde_json::to_string(&merged).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_json_simple() {
        let mut a: Value = serde_json::from_str(r#"{"x": 1, "y": 2}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"y": 99, "z": 3}"#).unwrap();
        merge_json(&mut a, b);
        assert_eq!(a["x"], 1);
        assert_eq!(a["y"], 99);
        assert_eq!(a["z"], 3);
    }

    #[test]
    fn test_merge_json_nested() {
        let mut a: Value = serde_json::from_str(r#"{"srv": {"port": 80, "host": "a"}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"srv": {"port": 443}}"#).unwrap();
        merge_json(&mut a, b);
        assert_eq!(a["srv"]["port"], 443);
        assert_eq!(a["srv"]["host"], "a");
    }

    #[test]
    fn test_merge_json_array_replaces() {
        let mut a: Value = serde_json::from_str(r#"{"list": [1, 2, 3]}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"list": [9]}"#).unwrap();
        merge_json(&mut a, b);
        assert_eq!(a["list"], serde_json::json!([9]));
    }

    #[test]
    fn test_cascade_missing_files_skipped() {
        // All missing — returns empty object
        let result = __varg_config_load_cascade(&[
            "/nonexistent/a.json".to_string(),
            "/nonexistent/b.json".to_string(),
        ]);
        assert_eq!(result.unwrap(), "{}");
    }

    #[test]
    fn test_cascade_roundtrip() {
        use std::io::Write;
        let dir = std::env::temp_dir();
        let a_path = dir.join("varg_wave29_cascade_a.json");
        let b_path = dir.join("varg_wave29_cascade_b.json");

        {
            let mut f = std::fs::File::create(&a_path).unwrap();
            f.write_all(br#"{"name": "base", "count": 1}"#).unwrap();
        }
        {
            let mut f = std::fs::File::create(&b_path).unwrap();
            f.write_all(br#"{"count": 42, "extra": true}"#).unwrap();
        }

        let merged = __varg_config_load_cascade(&[
            a_path.to_string_lossy().to_string(),
            b_path.to_string_lossy().to_string(),
        ])
        .unwrap();
        let v: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(v["name"], "base");
        assert_eq!(v["count"], 42);
        assert_eq!(v["extra"], true);

        let _ = std::fs::remove_file(&a_path);
        let _ = std::fs::remove_file(&b_path);
    }

    #[test]
    fn test_cascade_parse_error() {
        use std::io::Write;
        let path = std::env::temp_dir().join("varg_wave29_cascade_bad.json");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(b"not json at all").unwrap();
        }
        let result = __varg_config_load_cascade(&[path.to_string_lossy().to_string()]);
        assert!(result.is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_dirs_return_nonempty_on_test_host() {
        // On CI/dev hosts these should generally be populated. We assert the
        // calls don't panic — empty is a valid response in exotic envs.
        let _ = __varg_home_dir();
        let _ = __varg_config_dir();
        let _ = __varg_data_dir();
        let _ = __varg_cache_dir();
    }
}
