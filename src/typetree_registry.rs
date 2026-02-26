use std::path::PathBuf;

/// Resolve TypeTree registry paths with deterministic precedence:
/// 1) explicit CLI paths
/// 2) env `STITCHLANDS_TYPETREE_REGISTRY` (path-list)
/// 3) common local cache/workspace paths
pub fn resolve_typetree_registry_paths(explicit: &[PathBuf], auto_typetree: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();

    for path in explicit {
        if is_registry_file(path) && path.exists() {
            out.push(path.clone());
        }
    }

    if auto_typetree {
        if let Ok(value) = std::env::var("STITCHLANDS_TYPETREE_REGISTRY") {
            for candidate in split_path_list(&value) {
                if is_registry_file(&candidate) && candidate.exists() {
                    out.push(candidate);
                }
            }
        }

        for candidate in auto_candidates() {
            if is_registry_file(&candidate) && candidate.exists() {
                out.push(candidate);
            }
        }
    }

    dedupe_paths(&mut out);
    out
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
}
