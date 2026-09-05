//! Atomic artifact-directory writes.
//!
//! Report renderers decide what to emit. This module owns only the filesystem
//! installation boundary shared by those renderers and executable containers.

use std::fs;
use std::path::{Path, PathBuf};

use diagnostics::Diagnostic;
#[cfg(test)]
use executable_installation::{Artifact, ContainerLimits, encode_executable_container};

use super::{html_report, temp_path_for};

pub struct ArtifactWriter {
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

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_text(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        let path = self.root.join(file_name);
        let temp_path = temp_path_for(&path);
        let _ = fs::remove_file(&temp_path);
        fs::write(&temp_path, contents).map_err(|error| {
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
        })
    }

    pub fn write_html_report(
        &self,
        file_name: &str,
        title: &str,
        contents: &str,
    ) -> Result<(), Diagnostic> {
        self.write_text(file_name, &html_report(title, contents))
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

    /// Test-only packaging adapter for the quarantined executable-installation
    /// experiment. Ordinary artifact/report writing accepts already-produced
    /// bytes and has no installation-runtime dependency.
    ///
    /// Packages one already-normalized executable artifact in Omega's
    /// canonical semantic container.
    ///
    /// This is deliberately downstream of artifact construction and upstream
    /// of any target firmware envelope. It accepts neither a native image nor
    /// arbitrary bytes pretending to be code. The encoder revalidates its own
    /// output before this writer installs the file atomically.
    #[cfg(test)]
    pub(crate) fn write_executable_container(
        &self,
        file_name: &str,
        artifact: &Artifact,
        proof: &[u8],
        limits: ContainerLimits,
    ) -> Result<PathBuf, Diagnostic> {
        let bytes = encode_executable_container(artifact, proof, limits)
            .map_err(|diagnostic| Diagnostic::error(diagnostic.0))?;
        self.write_bytes(file_name, &bytes)
    }

    pub fn remove_files<'a>(
        &self,
        file_names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), Diagnostic> {
        for file_name in file_names {
            let path = self.root.join(file_name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Diagnostic::error(format!(
                        "failed to remove stale artifact {}: {error}",
                        path.display()
                    )));
                }
            }
        }

        Ok(())
    }
}
