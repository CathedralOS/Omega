use omega_executable_installation::{ArtifactId, InstalledCode, InstalledCodeId};

use crate::{
    TerminalExecutableImage, TerminalInstallationRecord, TerminalObjectArtifact,
    validate_terminal_installation_record,
};

/// Exact join between one canonical terminal installation record and the
/// installed code occurrence containing that record's compiler-authored text.
///
/// The record's fingerprints remain report identities. This opaque carrier is
/// produced only after replaying the complete record against the emitted image
/// and comparing both unrelocated and materialized text with `InstalledCode`.
#[derive(Debug)]
pub struct InstalledTerminalArtifact {
    installation: TerminalInstallationRecord,
    installed: InstalledCode,
}

impl InstalledTerminalArtifact {
    pub const fn installation(&self) -> &TerminalInstallationRecord {
        &self.installation
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed.identity()
    }

    pub fn artifact(&self) -> ArtifactId {
        self.installed.artifact()
    }

    /// Borrow the linear installed-code occurrence retained by this join.
    /// Callers cannot retire it while the joined artifact remains live.
    pub const fn installed(&self) -> &InstalledCode {
        &self.installed
    }

    /// Compare complete opaque installation evidence, not only report IDs.
    pub fn binds_installed_code(&self, installed: &InstalledCode) -> bool {
        self.installed.receipt_context() == installed.receipt_context()
    }

    /// Release the canonical installation record and exact installed-code
    /// custody after the higher-level runnable lifecycle has retired.
    pub fn into_parts(self) -> (TerminalInstallationRecord, InstalledCode) {
        (self.installation, self.installed)
    }
}

/// Transactional rejection from the terminal-record/artifact join. The
/// canonical record is returned so orchestration may correct another input and
/// retry without reconstructing installation metadata.
#[derive(Debug)]
pub struct InstalledTerminalArtifactBindingError {
    installation: TerminalInstallationRecord,
    installed: InstalledCode,
    diagnostic: String,
}

impl InstalledTerminalArtifactBindingError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (TerminalInstallationRecord, InstalledCode) {
        (self.installation, self.installed)
    }
}

impl std::fmt::Display for InstalledTerminalArtifactBindingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for InstalledTerminalArtifactBindingError {}

/// Bind canonical terminal installation metadata to one exact installed code
/// occurrence. Neither an image fingerprint nor an artifact ID can substitute
/// for the byte-bearing values replayed here.
pub fn bind_installed_terminal_artifact(
    object: &TerminalObjectArtifact,
    image: &TerminalExecutableImage,
    installation: TerminalInstallationRecord,
    installed: InstalledCode,
) -> Result<InstalledTerminalArtifact, Box<InstalledTerminalArtifactBindingError>> {
    let reject = |installation, installed, diagnostic: String| {
        Err(Box::new(InstalledTerminalArtifactBindingError {
            installation,
            installed,
            diagnostic,
        }))
    };

    if object.terminal_psi() != image.terminal_psi() || object.target() != image.target() {
        return reject(
            installation,
            installed,
            "terminal object and executable image have different semantic or target identity"
                .into(),
        );
    }
    if let Err(error) = validate_terminal_installation_record(&installation, image) {
        return reject(
            installation,
            installed,
            format!("terminal installation record does not bind the exact image: {error}"),
        );
    }
    if installed.architecture() != object.target().architecture {
        return reject(
            installation,
            installed,
            "installed code architecture differs from the terminal artifact target".into(),
        );
    }
    let Some(final_compiler_text) = image
        .output()
        .final_text_bytes
        .get(..object.text_bytes().len())
    else {
        return reject(
            installation,
            installed,
            "terminal executable image truncates the compiler-authored object text".into(),
        );
    };
    if !installed.binds_exact_materialized_artifact_bytes(object.text_bytes(), final_compiler_text)
    {
        return reject(
            installation,
            installed,
            "installed code does not contain the exact unrelocated and materialized terminal text"
                .into(),
        );
    }

    Ok(InstalledTerminalArtifact {
        installation,
        installed,
    })
}
