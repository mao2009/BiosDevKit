//! Resolve opaque artifact IDs through an explicit mapping under a trusted root.
//! This is for a filesystem NOT concurrently writable by an adversary. Race-safe
//! resolution against attacker-controlled renames needs handle-relative OS APIs;
//! no such guarantee is claimed. Reject symlinks anywhere in the path.
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};

fn safe_components(path: &Path) -> Result<Vec<std::ffi::OsString>, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(piece) => parts.push(piece.to_os_string()),
            _ => return Err("absolute, traversal, or non-normal path".into()),
        }
    }
    if parts.is_empty() {
        return Err("empty relative path".into());
    }
    Ok(parts)
}

/// Resolve a declared logical identifier, never interpreting the ID as a path.
/// Caller must ensure the root and its directory contents cannot be modified
/// concurrently by untrusted processes while resolving/opening.
pub fn open_mapped(
    root: &Path,
    mapping: &HashMap<String, PathBuf>,
    logical: &str,
) -> Result<File, String> {
    if logical.is_empty() {
        return Err("empty logical identifier".into());
    }
    let rel = mapping
        .get(logical)
        .ok_or("undeclared logical identifier")?;
    let pieces = safe_components(rel)?;
    let root_md = fs::symlink_metadata(root).map_err(|e| format!("root: {e}"))?;
    if root_md.file_type().is_symlink() || !root_md.is_dir() {
        return Err("root is symlink or not directory".into());
    }
    let root_canonical = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let mut cursor = root.to_path_buf();
    for (idx, piece) in pieces.iter().enumerate() {
        cursor.push(piece);
        let md = fs::symlink_metadata(&cursor).map_err(|e| format!("{}: {e}", cursor.display()))?;
        if md.file_type().is_symlink() {
            return Err("symlink component rejected".into());
        }
        if idx + 1 < pieces.len() && !md.is_dir() {
            return Err("intermediate component not directory".into());
        }
        if idx + 1 == pieces.len() && !md.is_file() {
            return Err("artifact is not a regular file".into());
        }
    }
    let resolved = fs::canonicalize(&cursor).map_err(|e| e.to_string())?;
    if !resolved.starts_with(&root_canonical) {
        return Err("path escapes root".into());
    }
    let file = File::open(&cursor).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    let path_md = fs::symlink_metadata(&cursor).map_err(|e| e.to_string())?;
    if !opened.is_file() || !path_md.is_file() || !(opened.len() == path_md.len()) {
        return Err("artifact changed during open".into());
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp() -> PathBuf {
        let t = std::env::temp_dir().join(format!(
            "bdk-resolver-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&t).unwrap();
        t
    }

    #[test]
    fn mapping_accepts_regular_and_rejects_attacks() {
        let root = tmp();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub/data.bin"), b"synthetic").unwrap();
        let mut map = HashMap::new();
        map.insert("opaque".into(), PathBuf::from("sub/data.bin"));
        assert!(open_mapped(&root, &map, "opaque").is_ok());
        assert!(open_mapped(&root, &map, "unknown").is_err());
        for bad in [
            "../outside",
            "/tmp/outside",
            "sub/../data.bin",
            "",
            "sub/nope",
        ] {
            map.insert("opaque".into(), PathBuf::from(bad));
            assert!(open_mapped(&root, &map, "opaque").is_err(), "{bad}");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_directory_and_missing_file() {
        let root = tmp();
        fs::create_dir(root.join("dir")).unwrap();
        let map = HashMap::from([("id".into(), PathBuf::from("dir"))]);
        assert!(open_mapped(&root, &map, "id").is_err());
        assert!(open_mapped(&root, &map, "unmapped").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_root_and_intermediate_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tmp();
        let alias = root.with_extension("alias");
        fs::create_dir(root.join("inside")).unwrap();
        fs::write(root.join("inside/data"), b"data").unwrap();
        symlink(&root, &alias).unwrap();
        let mapping = HashMap::from([("id".into(), PathBuf::from("inside/data"))]);
        assert!(open_mapped(&alias, &mapping, "id").is_err());
        symlink(root.join("inside"), root.join("other")).unwrap();
        let submap = HashMap::from([("id".into(), PathBuf::from("other/data"))]);
        assert!(open_mapped(&root, &submap, "id").is_err());
        fs::remove_file(alias).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = tmp();
        fs::write(root.join("data"), b"x").unwrap();
        symlink("data", root.join("link")).unwrap();
        let map = HashMap::from([("opaque".into(), PathBuf::from("link"))]);
        assert!(open_mapped(&root, &map, "opaque").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
