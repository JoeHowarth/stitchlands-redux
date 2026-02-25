use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

pub fn resolve_data_dir(input: &Path) -> Result<PathBuf> {
    let candidates = [
        input.to_path_buf(),
        input.join("Data"),
        input.join("RimWorldMac.app").join("Data"),
        input
            .join("RimWorldMac.app")
            .join("Contents")
            .join("Resources")
            .join("Data"),
        input.join("Contents").join("Resources").join("Data"),
    ];

    for candidate in candidates {
        if is_valid_data_dir(&candidate) {
            return Ok(candidate);
        }
    }

    bail!(
        "could not locate RimWorld Data dir from '{}'. Expected one of: '<path>/Data', '<path>/RimWorldMac.app/Data', '<path>/RimWorldMac.app/Contents/Resources/Data', or '<path>/Contents/Resources/Data'",
        input.display()
    )
}

fn is_valid_data_dir(path: &Path) -> bool {
    path.join("Core").join("Defs").exists()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn accepts_data_dir_directly() {
        let root = std::env::temp_dir().join(format!("stitchlands-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Core/Defs")).unwrap();
        fs::create_dir_all(root.join("Core/Textures")).unwrap();

        let found = resolve_data_dir(&root).unwrap();
        assert_eq!(found, root);

        let _ = fs::remove_dir_all(found);
    }

    #[test]
    fn resolves_from_install_root() {
        let root =
            std::env::temp_dir().join(format!("stitchlands-test-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let data = root
            .join("RimWorldMac.app")
            .join("Contents")
            .join("Resources")
            .join("Data");
        fs::create_dir_all(data.join("Core/Defs")).unwrap();
        fs::create_dir_all(data.join("Core/Textures")).unwrap();

        let found = resolve_data_dir(&root).unwrap();
        assert_eq!(found, data);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_from_steam_mac_root_layout() {
        let root = std::env::temp_dir().join(format!(
            "stitchlands-test-steam-layout-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let data = root.join("RimWorldMac.app").join("Data");
        fs::create_dir_all(data.join("Core/Defs")).unwrap();

        let found = resolve_data_dir(&root).unwrap();
        assert_eq!(found, data);

        let _ = fs::remove_dir_all(root);
    }
}
