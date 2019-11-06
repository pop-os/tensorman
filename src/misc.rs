use std::path::{Path, PathBuf};

/// Walks up the directory tree to find a file
pub fn walk_parent_directories(origin: &Path, file: &str) -> Option<PathBuf> {
    let mut next = Some(origin);

    while let Some(parent) = next {
        let config = parent.join(file);

        if config.exists() {
            return Some(config);
        }

        next = parent.parent();
    }

    None
}
