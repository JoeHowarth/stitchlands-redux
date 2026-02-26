use std::path::PathBuf;

pub fn resolve_rimworld_input(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path);
    }

    if let Ok(path) = std::env::var("STITCHLANDS_RIMWORLD_DATA") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    if let Ok(path) = std::env::var("RIMWORLD_DATA_DIR") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    common_rimworld_candidates()
        .into_iter()
        .find(|candidate| candidate.exists())
}

pub fn merge_path_list(explicit: &[PathBuf], env_var: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Ok(value) = std::env::var(env_var) {
        out.extend(split_path_list(&value));
    }
    out.extend(explicit.iter().cloned());
    dedupe_paths(&mut out);
    out
}

pub fn default_packed_index_path() -> PathBuf {
    if let Ok(path) = std::env::var("STITCHLANDS_PACKED_INDEX_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("stitchlands-redux")
            .join("packed_texture_index_v2.txt");
    }

    PathBuf::from(".stitchlands-packed-index-v2.txt")
}

fn common_rimworld_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("RimWorld"),
        );
        candidates.push(
            home.join(".local")
                .join("share")
                .join("Steam")
                .join("steamapps")
                .join("common")
                .join("RimWorld"),
        );
    }

    candidates
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
    fn path_list_merges_and_dedupes() {
        let merged = merge_path_list(
            &[PathBuf::from("a"), PathBuf::from("b")],
            "__MISSING_STITCHLANDS_VAR__",
        );
        assert_eq!(merged, vec![PathBuf::from("a"), PathBuf::from("b")]);
    }
}
