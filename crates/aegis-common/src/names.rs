//! Validation for names that become file names on disk.
//!
//! Collection, database and backup names are joined onto the data directory to
//! form file paths (`documents/<name>.json`, `databases/<name>.json`,
//! `backups/<id>/`). A name with a separator or a `..` component would let a
//! request read, overwrite or delete files outside that directory, so every
//! such name goes through [`validate_name`] before it is accepted, and every
//! path built from one goes through [`contained_path`].

use std::path::{Component, Path, PathBuf};

/// Longest accepted name, in bytes.
pub const MAX_NAME_LEN: usize = 128;

/// Why a name was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("name must not be empty")]
    Empty,
    #[error("name must be at most {MAX_NAME_LEN} characters")]
    TooLong,
    #[error("name may contain only letters, digits, '_', '-' and '.'")]
    InvalidCharacter,
    #[error("name must not be '.' or '..'")]
    Reserved,
}

/// Accept `^[A-Za-z0-9_.-]{1,128}$`, excluding `.` and `..`.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > MAX_NAME_LEN {
        return Err(NameError::TooLong);
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(NameError::InvalidCharacter);
    }
    if name == "." || name == ".." {
        return Err(NameError::Reserved);
    }
    Ok(())
}

/// True if [`validate_name`] accepts `name`.
pub fn is_valid_name(name: &str) -> bool {
    validate_name(name).is_ok()
}

/// Build `base/<name><suffix>` and confirm it stays directly inside `base`.
///
/// This is the containment check for building paths, not the naming rule:
/// it also accepts names that predate [`validate_name`] (for example with a
/// space) so existing data keeps persisting, but never a separator, NUL, `.`
/// or `..`. The result is checked lexically (one normal component) and, when
/// `base` exists, against the canonicalized base, and an existing entry must
/// not be a symlink.
pub fn contained_path(base: &Path, name: &str, suffix: &str) -> Result<PathBuf, NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > 255 {
        return Err(NameError::TooLong);
    }
    if name.contains(['/', '\\', '\0']) {
        return Err(NameError::InvalidCharacter);
    }
    if name == "." || name == ".." {
        return Err(NameError::Reserved);
    }
    let file_name = format!("{name}{suffix}");
    let mut components = Path::new(&file_name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => return Err(NameError::InvalidCharacter),
    }
    let path = base.join(&file_name);
    if let Ok(canon_base) = base.canonicalize() {
        let parent_ok = path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .is_some_and(|p| p == canon_base);
        if !parent_ok {
            return Err(NameError::InvalidCharacter);
        }
        // An existing entry must not be a symlink pointing elsewhere.
        if let Ok(meta) = std::fs::symlink_metadata(&path) {
            if meta.file_type().is_symlink() {
                return Err(NameError::InvalidCharacter);
            }
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_names() {
        for n in [
            "controllers",
            "equipment_commands",
            "org-1.v2",
            "A",
            "a..b",
            "...",
            &"x".repeat(MAX_NAME_LEN),
        ] {
            assert!(is_valid_name(n), "{n}");
        }
    }

    #[test]
    fn rejects_traversal_and_separators() {
        for n in [
            "",
            ".",
            "..",
            "../etc/passwd",
            "a/b",
            "a\\b",
            "/abs",
            "nul\0byte",
            "space name",
            "uni\u{e9}",
            &"x".repeat(MAX_NAME_LEN + 1),
        ] {
            assert!(!is_valid_name(n), "{n:?}");
        }
    }

    #[test]
    fn contained_path_stays_in_base() {
        let dir = std::env::temp_dir().join(format!("aegis-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = contained_path(&dir, "users", ".json").unwrap();
        assert_eq!(p, dir.join("users.json"));
        assert!(contained_path(&dir, "..", ".json").is_err());
        assert!(contained_path(&dir, "../x", ".json").is_err());
        assert!(contained_path(&dir, "a\\b", ".json").is_err());
        assert!(contained_path(&dir, "", ".json").is_err());
        // A legacy name that new collections may not use still persists.
        assert!(contained_path(&dir, "legacy name", ".json").is_ok());
        #[cfg(unix)]
        {
            let link = dir.join("evil.json");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink("/etc/passwd", &link).unwrap();
            assert!(contained_path(&dir, "evil", ".json").is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
