//! Staging, ordered installation and exact-byte replay of output files.
//!
//! Callers choose destinations and stage every member before installation.
//! Failure removes newly installed members and reports cleanup failures; it
//! does not restore overwritten files or provide a multi-file transaction.
//! This is separate from capability-relative, custody-checked record writes.
//! It neither selects compiler products nor interprets proof companions.

/// Publish a single non-executable file, such as a completed build output.
/// Artifact/proof pairs use the staged-set entrance so both files are ready
/// before installation starts.
pub fn publish_exact_file_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let product = stage_exact_bytes(path, bytes, false)?;
    install_staged_files(std::slice::from_ref(&product))
}

/// One file written and replayed beside its destination, not yet
/// visible there.
///
/// Stage all members of a related set before installing any of them, so a
/// staging failure leaves their previous destinations untouched. Dropping a
/// staged file may leave staging residue; this is not a rollback transaction.
pub struct StagedFile {
    staged: std::path::PathBuf,
    destination: std::path::PathBuf,
    /// Retained for the post-install replay. Owned rather than borrowed so a
    /// staged set can outlive the scopes that produced each member's bytes.
    bytes: Vec<u8>,
}

/// Write `bytes` beside `path` and replay them, without making them visible
/// at `path`.
///
/// Callers must serialize publication to each destination: the staging name
/// is process-scoped, not unique to concurrent attempts.
pub fn stage_exact_bytes(
    path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<StagedFile, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "native publication path has no parent directory".to_owned())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega-output");
    let staged = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("failed to stage {}: {error}", staged.display()))?;
    let staged_bytes = std::fs::read(&staged)
        .map_err(|error| format!("failed to replay {}: {error}", staged.display()))?;
    if staged_bytes != bytes {
        let _ = std::fs::remove_file(&staged);
        return Err("staged native output bytes failed exact replay".to_owned());
    }
    if executable && let Err(error) = make_executable(&staged) {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    Ok(StagedFile {
        staged,
        destination: path.to_path_buf(),
        bytes: bytes.to_vec(),
    })
}

fn replay_installed_product(product: &StagedFile) -> Result<(), String> {
    let installed = std::fs::read(&product.destination).map_err(|error| {
        format!(
            "failed to replay {}: {error}",
            product.destination.display()
        )
    })?;
    if installed != product.bytes {
        return Err("published native output bytes failed exact replay".to_owned());
    }
    Ok(())
}

/// Install and replay a fully staged set in its supplied order.
///
/// Staging completes before installation starts. On installation or replay
/// failure, remove the members this attempt installed so new artifact bytes
/// cannot remain beside a previous companion. Untouched destinations survive;
/// replaced files are not restored. Cleanup failures accompany the original
/// error, and no receipt may be issued for this attempt.
pub fn install_staged_files(products: &[StagedFile]) -> Result<(), String> {
    let mut installed_count = 0;
    for product in products {
        let installation = install_staged(&product.staged, &product.destination).and_then(|()| {
            installed_count += 1;
            replay_installed_product(product)
        });
        if let Err(mut error) = installation {
            for installed_product in &products[..installed_count] {
                match std::fs::remove_file(&installed_product.destination) {
                    Ok(()) => {}
                    Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(cleanup_error) => error.push_str(&format!(
                        "; failed to remove incomplete publication {}: {cleanup_error}",
                        installed_product.destination.display()
                    )),
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

/// Install the staged file at its destination. Unix `rename` replaces an
/// existing file atomically; platforms whose rename refuses an occupied
/// destination remove it first. Either way the staged bytes were already
/// replayed, and on Unix the prior output survives a failed rename.
#[cfg(unix)]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    }
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

/// Set Unix executable permissions to 0755; other hosts need no mode change.
#[cfg(unix)]
pub fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

/// Set Unix executable permissions to 0755; other hosts need no mode change.
#[cfg(not(unix))]
pub fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}
