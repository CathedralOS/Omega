//! Filesystem publication of an already finalized runnable component.
//!
//! Validate installation, stage and replay the sealed bytes and mode, rename,
//! then replay the visible file. Success retains runnable custody and its
//! non-clonable receipt; failure returns the original runnable and path.

use component_publication::InstalledRunnableComponent;
use image_emission::{installation_fingerprint, validate_installation_record};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FLAT_OUTPUT_STAGING_IDENTITY: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod tests;

/// Publish the exact executable image retained by a finalized deployment.
///
/// The requested filename must equal the compiler-sealed image filename. The
/// file is staged beside its destination, replayed byte-for-byte with its
/// executable mode in place, atomically renamed, and replayed again before a
/// receipt exists. Any failure returns the complete runnable carrier.
pub fn publish_component_flat_output(
    runnable: InstalledRunnableComponent,
    requested_path: PathBuf,
) -> Result<PublishedComponentFlatOutput, Box<ComponentFlatOutputPublicationError>> {
    let result = publish_component_image(&runnable, &requested_path);
    match result {
        Ok(receipt) => Ok(PublishedComponentFlatOutput { runnable, receipt }),
        Err(diagnostic) => Err(Box::new(ComponentFlatOutputPublicationError {
            runnable,
            requested_path,
            diagnostic,
        })),
    }
}

fn publish_component_image(
    runnable: &InstalledRunnableComponent,
    output_path: &Path,
) -> Result<ComponentFlatOutputReceipt, String> {
    let terminal = runnable.installed_artifact();
    let image = terminal.image();
    let installation = terminal.installation();
    validate_installation_record(installation, image)
        .map_err(|error| format!("terminal output installation replay failed: {error}"))?;
    let installation_fingerprint = installation_fingerprint(installation)
        .map_err(|error| format!("terminal output installation fingerprint failed: {error}"))?;
    let output = image.output();
    if output_path.file_name() != Some(std::ffi::OsStr::new(&output.file_name)) {
        return Err(format!(
            "terminal output path must retain sealed executable filename `{}`",
            output.file_name
        ));
    }
    if output.bytes.is_empty() {
        return Err("terminal output image cannot publish empty executable bytes".into());
    }

    write_atomic_executable(output_path, &output.bytes)?;
    let receipt = ComponentFlatOutputReceipt {
        output_path: output_path.to_path_buf(),
        installation_fingerprint,
        image_fingerprint: installation.image(),
        byte_count: output.bytes.len(),
    };
    if let Err(diagnostic) = validate_terminal_flat_output_receipt(&receipt, runnable) {
        let _ = std::fs::remove_file(output_path);
        return Err(diagnostic);
    }
    Ok(receipt)
}

/// Exact filesystem-publication receipt for one deployed terminal component.
///
/// The receipt is deliberately non-clonable. Its installation fingerprint
/// commits the canonical manifest and acceptance identities, while its image
/// fingerprint commits the exact bytes replayed at `output_path`.
#[derive(Debug)]
#[must_use = "terminal component output publication receipts must remain with deployment custody"]
pub struct ComponentFlatOutputReceipt {
    output_path: PathBuf,
    installation_fingerprint: image_emission::InstallationFingerprint,
    image_fingerprint: image_emission::ImageFingerprint,
    byte_count: usize,
}

impl ComponentFlatOutputReceipt {
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    pub const fn installation_fingerprint(&self) -> image_emission::InstallationFingerprint {
        self.installation_fingerprint
    }

    pub const fn image_fingerprint(&self) -> image_emission::ImageFingerprint {
        self.image_fingerprint
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }

    /// Replay this receipt against both the retained deployment evidence and
    /// the currently visible file.
    pub fn validate(
        &self,
        runnable: &InstalledRunnableComponent,
    ) -> Result<(), ComponentFlatOutputValidationError> {
        validate_terminal_flat_output_receipt(self, runnable)
            .map_err(ComponentFlatOutputValidationError)
    }
}

/// A visible flat executable that still owns the complete runnable component.
/// Later era publication can recover both values explicitly; a filesystem
/// receipt never decomposes or substitutes for installation custody.
#[derive(Debug)]
#[must_use = "published terminal output retains runnable installation custody"]
pub struct PublishedComponentFlatOutput {
    runnable: InstalledRunnableComponent,
    receipt: ComponentFlatOutputReceipt,
}

impl PublishedComponentFlatOutput {
    pub const fn runnable(&self) -> &InstalledRunnableComponent {
        &self.runnable
    }

    pub const fn receipt(&self) -> &ComponentFlatOutputReceipt {
        &self.receipt
    }

    pub fn validate(&self) -> Result<(), ComponentFlatOutputValidationError> {
        self.receipt.validate(&self.runnable)
    }

    /// Recover both the live runnable carrier and its filesystem receipt for
    /// transfer to the next deployment/era owner.
    pub fn into_parts(self) -> (InstalledRunnableComponent, ComponentFlatOutputReceipt) {
        (self.runnable, self.receipt)
    }
}

#[derive(Debug)]
pub struct ComponentFlatOutputPublicationError {
    runnable: InstalledRunnableComponent,
    requested_path: PathBuf,
    diagnostic: String,
}

impl ComponentFlatOutputPublicationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn requested_path(&self) -> &Path {
        &self.requested_path
    }

    /// Recover the exact runnable custody and caller-requested path after any
    /// rejected or failed publication attempt.
    pub fn into_parts(self) -> (InstalledRunnableComponent, PathBuf) {
        (self.runnable, self.requested_path)
    }
}

impl std::fmt::Display for ComponentFlatOutputPublicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentFlatOutputPublicationError {}

#[derive(Debug, PartialEq, Eq)]
pub struct ComponentFlatOutputValidationError(String);

impl ComponentFlatOutputValidationError {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentFlatOutputValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ComponentFlatOutputValidationError {}

fn validate_terminal_flat_output_receipt(
    receipt: &ComponentFlatOutputReceipt,
    runnable: &InstalledRunnableComponent,
) -> Result<(), String> {
    let terminal = runnable.installed_artifact();
    validate_installation_record(terminal.installation(), terminal.image())
        .map_err(|error| format!("terminal output installation replay failed: {error}"))?;
    let installation_fingerprint = installation_fingerprint(terminal.installation())
        .map_err(|error| format!("terminal output installation fingerprint failed: {error}"))?;
    if receipt.installation_fingerprint != installation_fingerprint
        || receipt.image_fingerprint != terminal.installation().image()
        || receipt.byte_count != terminal.image().output().bytes.len()
        || receipt.output_path.file_name()
            != Some(std::ffi::OsStr::new(&terminal.image().output().file_name))
    {
        return Err(
            "terminal output receipt does not bind the exact runnable installation and image"
                .into(),
        );
    }
    validate_published_executable(&receipt.output_path, &terminal.image().output().bytes)
}

fn write_atomic_executable(output_path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create terminal output directory {}: {error}",
            parent.display()
        )
    })?;
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "terminal output path has no UTF-8 executable filename".to_owned())?;
    let staging_identity = NEXT_FLAT_OUTPUT_STAGING_IDENTITY.fetch_add(1, Ordering::Relaxed);
    let staged_path = output_path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        staging_identity
    ));
    let mut staged = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged_path)
        .map_err(|error| {
            format!(
                "failed to create staged terminal output {}: {error}",
                staged_path.display()
            )
        })?;
    if let Err(error) = staged.write_all(bytes) {
        drop(staged);
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to write staged terminal output {}: {error}",
            staged_path.display()
        ));
    }
    if let Err(error) = staged.sync_all() {
        drop(staged);
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to synchronize staged terminal output {}: {error}",
            staged_path.display()
        ));
    }
    drop(staged);
    if let Err(diagnostic) = mark_executable(&staged_path)
        .and_then(|()| validate_published_executable(&staged_path, bytes))
    {
        let _ = std::fs::remove_file(&staged_path);
        return Err(diagnostic);
    }
    if let Err(error) = std::fs::rename(&staged_path, output_path) {
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "failed to install terminal output {}: {error}",
            output_path.display()
        ));
    }
    if let Err(diagnostic) = validate_published_executable(output_path, bytes) {
        let _ = std::fs::remove_file(output_path);
        return Err(diagnostic);
    }
    Ok(())
}

fn validate_published_executable(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = std::fs::read(path).map_err(|error| {
        format!(
            "failed to replay terminal output {}: {error}",
            path.display()
        )
    })?;
    if actual != expected {
        return Err("published terminal output bytes differ from the deployed image".into());
    }
    validate_executable_mode(path)
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| format!("failed to read {} permissions: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn validate_executable_mode(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)
        .map_err(|error| format!("failed to read {} permissions: {error}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o777 != 0o755 {
        return Err("published terminal output does not retain exact executable mode 0755".into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_executable_mode(_path: &Path) -> Result<(), String> {
    Ok(())
}
