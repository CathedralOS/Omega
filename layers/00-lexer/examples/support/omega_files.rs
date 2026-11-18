//! Finds `.omg` files under a directory for the example tools.

use std::path::{Path, PathBuf};

/// Adds every `.omg` file under `directory`, skipping build output and hidden directories.
///
/// A checkout can hold build output and other working copies of the same files, so directories
/// named `target` or starting with a dot are not entered.
pub fn collect_omega_files(directory: &Path, source_paths: &mut Vec<PathBuf>) {
    let Ok(directory_entries) = std::fs::read_dir(directory) else {
        eprintln!("cannot read directory {}", directory.display());
        return;
    };

    for directory_entry in directory_entries.flatten() {
        let entry_path = directory_entry.path();
        let entry_name = directory_entry.file_name();
        let entry_name = entry_name.to_string_lossy();
        if entry_path.is_dir() {
            if entry_name != "target" && !entry_name.starts_with('.') {
                collect_omega_files(&entry_path, source_paths);
            }
        } else if entry_path
            .extension()
            .is_some_and(|extension| extension == "omg")
        {
            source_paths.push(entry_path);
        }
    }
}
