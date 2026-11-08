use std::path::{Component, Path, PathBuf};

use crate::error::AppError;

pub fn resolve_book_path(root: &Path, rel: &str) -> Result<PathBuf, AppError> {
    if rel.is_empty() || rel.contains('\0') {
        return Err(AppError::NotFound);
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(AppError::NotFound);
    }
    for comp in rel_path.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return Err(AppError::NotFound),
        }
    }

    let canon_root = root.canonicalize().map_err(|_| AppError::NotFound)?;
    let mut current = canon_root.clone();
    for comp in rel_path.components() {
        current.push(comp);
        let meta = std::fs::symlink_metadata(&current).map_err(|_| AppError::NotFound)?;
        if meta.file_type().is_symlink() {
            return Err(AppError::NotFound);
        }
    }

    let canon_joined = current.canonicalize().map_err(|_| AppError::NotFound)?;
    if !canon_joined.starts_with(&canon_root) {
        return Err(AppError::NotFound);
    }

    Ok(canon_joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_valid_and_reject_invalid_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("books");
        fs::create_dir_all(root.join("serie")).unwrap();
        let book_file = root.join("serie/book.cbz");
        fs::write(&book_file, b"ok").unwrap();

        let outside = tmp.path().join("secret.txt");
        fs::write(&outside, b"secret").unwrap();

        let resolved = resolve_book_path(&root, "serie/book.cbz").unwrap();
        assert_eq!(resolved, book_file.canonicalize().unwrap());

        assert!(resolve_book_path(&root, "").is_err());
        assert!(resolve_book_path(&root, "serie\0/book.cbz").is_err());
        assert!(resolve_book_path(&root, "../secret.txt").is_err());
        assert!(resolve_book_path(&root, "serie/../../secret.txt").is_err());
        assert!(resolve_book_path(&root, "/etc/passwd").is_err());

        #[cfg(unix)]
        {
            let link_path = root.join("serie/link.cbz");
            std::os::unix::fs::symlink(&outside, &link_path).unwrap();
            assert!(resolve_book_path(&root, "serie/link.cbz").is_err());
        }
    }
}
