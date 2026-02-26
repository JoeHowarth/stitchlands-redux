use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use unity_asset_decode::typetree::{
    CompositeTypeTreeRegistry, JsonTypeTreeRegistry, TpkTypeTreeRegistry, TypeTreeRegistry,
};
use walkdir::WalkDir;

/// Resolve TypeTree registry paths with deterministic precedence:
/// 1) explicit CLI paths
/// 2) env `STITCHLANDS_TYPETREE_REGISTRY` (path-list)
/// 3) common local cache/workspace paths
pub fn resolve_typetree_registry_paths(explicit: &[PathBuf], auto_typetree: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for path in explicit {
        out.extend(expand_registry_inputs(path));
    }

    if auto_typetree {
        if let Ok(value) = std::env::var("STITCHLANDS_TYPETREE_REGISTRY") {
            for candidate in split_path_list(&value) {
                out.extend(expand_registry_inputs(&candidate));
            }
        }

        for candidate in auto_candidates() {
            out.extend(expand_registry_inputs(&candidate));
        }
    }

    dedupe_paths(&mut out);
    out
}

pub fn load_typetree_registry(paths: &[PathBuf]) -> Result<Option<Arc<dyn TypeTreeRegistry>>> {
    if paths.is_empty() {
        return Ok(None);
    }

    let mut registries: Vec<Arc<dyn TypeTreeRegistry>> = Vec::new();
    for path in paths {
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext == "tpk" {
            let registry = TpkTypeTreeRegistry::from_path(path)?;
            registries.push(Arc::new(registry));
        } else {
            let registry = JsonTypeTreeRegistry::from_path(path)?;
            registries.push(Arc::new(registry));
        }
    }

    if registries.len() == 1 {
        Ok(Some(registries.remove(0)))
    } else {
        Ok(Some(Arc::new(CompositeTypeTreeRegistry::new(registries))))
    }
}

fn auto_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    candidates.push(cwd.join("typetree").join("lz4.tpk"));
    candidates.push(cwd.join("typetree").join("default.tpk"));
    candidates.push(
        cwd.join("target")
            .join("investigation")
            .join("typetree")
            .join("lz4.tpk"),
    );

    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        candidates.push(
            home.join(".cache")
                .join("stitchlands-redux")
                .join("typetree")
                .join("lz4.tpk"),
        );
        candidates.push(
            home.join(".local")
                .join("share")
                .join("stitchlands-redux")
                .join("typetree")
                .join("lz4.tpk"),
        );
    }

    candidates
}

fn is_registry_file(path: &PathBuf) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ext == "tpk" || ext == "json"
}

fn expand_registry_inputs(path: &PathBuf) -> Vec<PathBuf> {
    if !path.exists() {
        return Vec::new();
    }
    if path.is_file() {
        if is_registry_file(path) {
            return vec![path.clone()];
        }
        return Vec::new();
    }
    if !path.is_dir() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let candidate = entry.path().to_path_buf();
        if is_registry_file(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn split_path_list(value: &str) -> Vec<PathBuf> {
    value
        .split([':', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut unique = Vec::new();
    for path in std::mem::take(paths) {
        if unique.iter().any(|p: &PathBuf| p == &path) {
            continue;
        }
        unique.push(path);
    }
    *paths = unique;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_tpk_and_json_extensions() {
        assert!(is_registry_file(&PathBuf::from("abc.tpk")));
        assert!(is_registry_file(&PathBuf::from("abc.json")));
        assert!(!is_registry_file(&PathBuf::from("abc.txt")));
    }

    #[test]
    fn splits_path_list() {
        let paths = split_path_list("a.tpk:b.json;c.tpk");
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn expands_directory_inputs() {
        let root =
            std::env::temp_dir().join(format!("stitchlands-typetree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("a.tpk"), "x").unwrap();
        std::fs::write(root.join("nested").join("b.json"), "{}").unwrap();
        std::fs::write(root.join("nested").join("c.txt"), "x").unwrap();

        let mut expanded = expand_registry_inputs(&root);
        expanded.sort();
        assert_eq!(expanded.len(), 2);

        let _ = std::fs::remove_dir_all(root);
    }
}
