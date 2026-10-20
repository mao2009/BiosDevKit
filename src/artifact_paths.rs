//! Resolve opaque artifact IDs through an explicit mapping under a trusted root.
//! This is for a filesystem NOT concurrently writable by an adversary. Race-safe
//! resolution against attacker-controlled renames needs handle-relative OS APIs;
//! no such guarantee is claimed. Reject symlinks anywhere in the path.
use std::collections::HashMap;
use std::fs::File;
#[cfg(test)]
use std::fs;
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

#[cfg(target_os = "linux")]
fn linux_open(root: &Path, pieces: &[std::ffi::OsString]) -> Result<File, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;

    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }
    unsafe extern "C" {
        fn syscall(number: std::ffi::c_long, ...) -> std::ffi::c_long;
    }
    // Linux openat2 ABI: syscall 437 on x86_64 and aarch64.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    const OPENAT2: std::ffi::c_long = 437;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    compile_error!("Linux openat2 syscall number has not been verified for this architecture");
    const O_CLOEXEC: u64 = 0o2000000;
    const O_DIRECTORY: u64 = 0o200000;
    const O_PATH: u64 = 0o10000000;
    const RESOLVE_NO_SYMLINKS: u64 = 0x04;
    const RESOLVE_BENEATH: u64 = 0x08;

    let root_path =
        CString::new(root.as_os_str().as_bytes()).map_err(|_| "root contains NUL".to_string())?;
    let root_how = OpenHow {
        flags: O_PATH | O_DIRECTORY | O_CLOEXEC,
        mode: 0,
        resolve: RESOLVE_NO_SYMLINKS,
    };
    // AT_FDCWD is -100. The root is opened atomically with no symlink traversal,
    // including its ancestors. A writable ancestor can change which root is
    // selected but cannot permit escape from the opened root descriptor.
    let root_fd = unsafe {
        syscall(
            OPENAT2,
            -100isize,
            root_path.as_ptr(),
            &root_how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "safe root open failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let root_handle = unsafe { File::from_raw_fd(root_fd as i32) };
    let relative = pieces.iter().map(|v| v.as_bytes()).collect::<Vec<_>>();
    let mut bytes = Vec::new();
    for (idx, part) in relative.iter().enumerate() {
        if idx > 0 {
            bytes.push(b'/');
        }
        bytes.extend_from_slice(part);
    }
    let rel_path = CString::new(bytes).map_err(|_| "mapped path contains NUL".to_string())?;
    let file_how = OpenHow {
        flags: O_CLOEXEC,
        mode: 0,
        resolve: RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS,
    };
    let fd = unsafe {
        syscall(
            OPENAT2,
            root_handle.as_raw_fd(),
            rel_path.as_ptr(),
            &file_how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if fd < 0 {
        return Err(format!(
            "safe artifact open failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let file = unsafe { File::from_raw_fd(fd as i32) };
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("artifact is not a regular file".into());
    }
    Ok(file)
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
    // Only Linux currently has a race-resistant implementation. Unsupported
    // platforms fail closed instead of returning a path-race-vulnerable handle.
    #[cfg(target_os = "linux")]
    {
        linux_open(root, &pieces)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, pieces);
        Err("race-safe artifact opening is unsupported on this platform".into())
    }
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
        #[cfg(target_os = "linux")]
        assert!(open_mapped(&root, &map, "opaque").is_ok());
        #[cfg(not(target_os = "linux"))]
        assert!(open_mapped(&root, &map, "opaque").is_err());
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
