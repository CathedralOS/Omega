//! Per-file publication in a compiler observation directory.
//!
//! Report renderers decide what to emit. This module owns only the filesystem
//! installation boundary shared by those renderers and executable containers.

use std::fs;
use std::path::{Path, PathBuf};

use diagnostics::Diagnostic;

pub(crate) struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    pub fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        let root = build_dir.to_path_buf();
        fs::create_dir_all(&root).map_err(|error| {
            Diagnostic::error(format!(
                "failed to create artifact directory {}: {error}",
                root.display()
            ))
        })?;

        Ok(Self { root })
    }

    pub fn write_text(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        self.write_bytes(file_name, contents.as_bytes()).map(|_| ())
    }

    pub fn write_bytes(&self, file_name: &str, bytes: &[u8]) -> Result<PathBuf, Diagnostic> {
        let path = self.root.join(file_name);
        let temp_path = temp_path_for(&path);
        let _ = fs::remove_file(&temp_path);
        fs::write(&temp_path, bytes).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write temporary artifact {}: {error}",
                temp_path.display()
            ))
        })?;
        fs::rename(&temp_path, &path).map_err(|error| {
            let _ = fs::remove_file(&temp_path);
            Diagnostic::error(format!(
                "failed to install artifact {}: {error}",
                path.display()
            ))
        })?;

        Ok(path)
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-artifact"),
        std::process::id()
    ))
}

#[cfg(test)]
mod tests;
