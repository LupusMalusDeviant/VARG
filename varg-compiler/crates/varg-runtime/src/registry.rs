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
    h.lock().unwrap().install(name, version)
}

pub fn __varg_registry_uninstall(h: &RegistryHandle, name: &str) -> bool {
    h.lock().unwrap().uninstall(name)
}

pub fn __varg_registry_is_installed(h: &RegistryHandle, name: &str) -> bool {
    h.lock().unwrap().is_installed(name)
}

pub fn __varg_registry_version(h: &RegistryHandle, name: &str) -> String {
    h.lock().unwrap().version(name)
}

pub fn __varg_registry_list(h: &RegistryHandle) -> Vec<String> {
    h.lock().unwrap().list()
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
