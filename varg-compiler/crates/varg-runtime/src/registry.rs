// Wave 34: Varg Package Registry Client

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct VargRegistry {
    installed: HashMap<String, String>, // name -> version
    cache_path: String,
}

impl VargRegistry {
    pub fn new(cache_path: &str) -> Self {
        let mut r = VargRegistry { installed: HashMap::new(), cache_path: cache_path.to_string() };
        r.load();
        r
    }

    fn index_path(&self) -> String {
        format!("{}/installed.json", self.cache_path)
    }

    fn load(&mut self) {
        if self.cache_path == ":memory_reg:" { return; }
        if let Ok(text) = std::fs::read_to_string(self.index_path()) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&text) {
                self.installed = map;
            }
        }
    }

    fn persist(&self) {
        if self.cache_path == ":memory_reg:" { return; }
        let _ = std::fs::create_dir_all(&self.cache_path);
        if let Ok(json) = serde_json::to_string(&self.installed) {
            let _ = std::fs::write(self.index_path(), json);
        }
    }

    pub fn install(&mut self, name: &str, version: &str) -> bool {
        self.installed.insert(name.to_string(), version.to_string());
        self.persist();
        true
    }

    pub fn uninstall(&mut self, name: &str) -> bool {
        let existed = self.installed.remove(name).is_some();
        if existed { self.persist(); }
        existed
    }

    pub fn is_installed(&self, name: &str) -> bool { self.installed.contains_key(name) }
    pub fn version(&self, name: &str) -> String { self.installed.get(name).cloned().unwrap_or_default() }
    pub fn list(&self) -> Vec<String> { self.installed.keys().cloned().collect() }
}

pub type RegistryHandle = Arc<Mutex<VargRegistry>>;

pub fn __varg_registry_open(cache_path: &str) -> RegistryHandle {
    Arc::new(Mutex::new(VargRegistry::new(cache_path)))
}

pub fn __varg_registry_install(h: &RegistryHandle, name: &str, version: &str) -> bool {
    h.lock().unwrap_or_else(|e| e.into_inner()).install(name, version)
}

pub fn __varg_registry_uninstall(h: &RegistryHandle, name: &str) -> bool {
    h.lock().unwrap_or_else(|e| e.into_inner()).uninstall(name)
}

pub fn __varg_registry_is_installed(h: &RegistryHandle, name: &str) -> bool {
    h.lock().unwrap_or_else(|e| e.into_inner()).is_installed(name)
}

pub fn __varg_registry_version(h: &RegistryHandle, name: &str) -> String {
    h.lock().unwrap_or_else(|e| e.into_inner()).version(name)
}

pub fn __varg_registry_list(h: &RegistryHandle) -> Vec<String> {
    h.lock().unwrap_or_else(|e| e.into_inner()).list()
}

// ── Real package download with checksum verification ──────────────────────────
//
// `registry_install` only records name→version metadata. `registry_download` actually fetches the
// artifact over HTTP and refuses to install it unless its SHA-256 matches the expected digest —
// an unverified download is how a package registry becomes a supply-chain hole, so a mismatch is a
// hard error and nothing is written or recorded.

/// Hex SHA-256 of a byte slice.
#[cfg(feature = "crypto")]
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Verify `bytes` against `expected_sha256` (hex, case-insensitive), then write the artifact into
/// the registry cache and record it as installed. Returns the written path.
///
/// Split out from the HTTP call so the security-critical path is testable without a network.
#[cfg(feature = "crypto")]
pub fn install_verified_bytes(
    h: &RegistryHandle,
    name: &str,
    version: &str,
    bytes: &[u8],
    expected_sha256: &str,
) -> Result<String, String> {
    let actual = sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
        return Err(format!(
            "checksum mismatch for {}@{}: expected {}, got {} — refusing to install",
            name, version, expected_sha256.trim(), actual
        ));
    }
    let mut reg = h.lock().unwrap_or_else(|e| e.into_inner());
    let cache = reg.cache_path.clone();
    if cache == ":memory_reg:" {
        // In-memory registry: verification still applies, but nothing is written to disk.
        reg.install(name, version);
        return Ok(String::new());
    }
    std::fs::create_dir_all(&cache).map_err(|e| format!("cannot create cache dir '{}': {}", cache, e))?;
    let path = format!("{}/{}-{}.pkg", cache, name, version);
    std::fs::write(&path, bytes).map_err(|e| format!("cannot write '{}': {}", path, e))?;
    reg.install(name, version);
    Ok(path)
}

/// Download a package over HTTP and install it only if its SHA-256 matches `expected_sha256`.
#[cfg(all(feature = "net", feature = "crypto"))]
pub fn __varg_registry_download(
    h: &RegistryHandle,
    name: &str,
    version: &str,
    url: &str,
    expected_sha256: &str,
) -> Result<String, String> {
    let resp = reqwest::blocking::get(url)
        .map_err(|e| format!("download failed for '{}': {}", url, e))?;
    if !resp.status().is_success() {
        return Err(format!("download failed for '{}': HTTP {}", url, resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| format!("reading '{}' failed: {}", url, e))?;
    install_verified_bytes(h, name, version, &bytes, expected_sha256)
}

/// Search the known package catalog (stub — real registry would HTTP-query an index).
pub fn __varg_registry_search(query: &str) -> Vec<String> {
    let catalog = [
        "varg-http-tools", "varg-db-tools", "varg-llm-tools",
        "varg-text-utils", "varg-json-utils", "varg-crypto-utils",
        "varg-file-utils", "varg-agent-templates", "varg-mcp-clients",
        "varg-vector-extra", "varg-graph-utils", "varg-workflow-dsl",
    ];
    catalog.iter()
        .filter(|p| query.is_empty() || p.contains(query))
        .map(|p| p.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> RegistryHandle { __varg_registry_open(":memory_reg:") }

    // ── Checksum-verified download ────────────────────────────────────────
    // Exercises the security-critical path without a network: only the HTTP GET is omitted.

    #[cfg(feature = "crypto")]
    #[test]
    fn sha256_hex_matches_known_vector() {
        // Well-known SHA-256 of "abc".
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn install_verified_bytes_accepts_matching_checksum() {
        let r = mem();
        let data = b"package-contents";
        let sha = sha256_hex(data);
        assert!(install_verified_bytes(&r, "pkg", "1.0.0", data, &sha).is_ok());
        assert!(__varg_registry_is_installed(&r, "pkg"));
        // Hex comparison is case-insensitive.
        assert!(install_verified_bytes(&r, "pkg2", "1.0.0", data, &sha.to_uppercase()).is_ok());
        assert!(__varg_registry_is_installed(&r, "pkg2"));
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn install_verified_bytes_rejects_tampered_payload_and_does_not_install() {
        let r = mem();
        let expected = sha256_hex(b"original");
        // Payload was swapped out — must be refused, and nothing recorded.
        let err = install_verified_bytes(&r, "evil", "1.0.0", b"tampered", &expected).unwrap_err();
        assert!(err.contains("checksum mismatch"), "got: {}", err);
        assert!(!__varg_registry_is_installed(&r, "evil"), "must not install on mismatch");
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn install_verified_bytes_writes_artifact_to_cache() {
        let dir = std::env::temp_dir().join("varg_registry_dl_test");
        let _ = std::fs::remove_dir_all(&dir);
        let r = __varg_registry_open(dir.to_str().unwrap());
        let data = b"real-bytes";
        let sha = sha256_hex(data);
        let path = install_verified_bytes(&r, "tool", "2.1.0", data, &sha).expect("should install");
        assert!(path.ends_with("tool-2.1.0.pkg"), "path: {}", path);
        assert_eq!(std::fs::read(&path).unwrap(), data, "artifact bytes must be written verbatim");
        assert!(__varg_registry_is_installed(&r, "tool"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_registry_install_and_list() {
        let r = mem();
        assert!(__varg_registry_install(&r, "varg-http-tools", "1.0.0"));
        assert!(__varg_registry_list(&r).contains(&"varg-http-tools".to_string()));
    }

    #[test]
    fn test_registry_is_installed() {
        let r = mem();
        assert!(!__varg_registry_is_installed(&r, "pkg"));
        __varg_registry_install(&r, "pkg", "2.0.0");
        assert!(__varg_registry_is_installed(&r, "pkg"));
    }

    #[test]
    fn test_registry_uninstall() {
        let r = mem();
        __varg_registry_install(&r, "pkg", "1.0.0");
        assert!(__varg_registry_uninstall(&r, "pkg"));
        assert!(!__varg_registry_is_installed(&r, "pkg"));
    }

    #[test]
    fn test_registry_version() {
        let r = mem();
        __varg_registry_install(&r, "pkg", "3.1.4");
        assert_eq!(__varg_registry_version(&r, "pkg"), "3.1.4");
    }

    #[test]
    fn test_registry_search_by_keyword() {
        let results = __varg_registry_search("http");
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.contains("http")));
    }

    #[test]
    fn test_registry_search_empty_returns_all() {
        assert!(__varg_registry_search("").len() > 5);
    }

    #[test]
    fn test_registry_uninstall_nonexistent_returns_false() {
        let r = mem();
        assert!(!__varg_registry_uninstall(&r, "not_there"));
    }
}
