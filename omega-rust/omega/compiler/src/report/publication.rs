//! Exact-byte publication for compiled products and completed build files.
//!
//! Prepare and validate requested pairs before staging. Stage every file before
//! `install_staged_products` makes destinations visible, then replay installed
//! bytes. A failed installation removes newly installed members and reports any
//! cleanup failures; it does not restore replaced files or provide a transaction.
//! `psi_pair` owns the common Terminal artifact/proof preparation and staging.
//! Native receipt identities and proof validation belong to `executable_publication`.

mod psi_pair;

pub(crate) use psi_pair::PreparedPsiProofPair;

use std::path::PathBuf;

/// Publish a single non-executable file, such as a completed build output.
/// Artifact/proof pairs use the staged-set entrance so both files are ready
/// before installation starts.
pub(crate) fn publish_exact_file_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let product = stage_exact_bytes(path, bytes, false)?;
    install_staged_products(std::slice::from_ref(&product))
}

/// One product file written and replayed beside its destination, not yet
/// visible there.
///
/// Publication of a PAIR cannot be a sequence of single-file publications:
/// "Publication must not associate a stale sidecar with newly written bytes"
/// (spec, `proofs/publication.md`), and writing the artifact and then its
/// companion leaves exactly that arrangement when the second write fails.
/// Staging every member first, then installing them, means a write failure
/// leaves the previous consistent pair untouched. The spec permits the
/// residue: "Failure may leave diagnostic staging data".
pub(crate) struct StagedProduct {
    staged: std::path::PathBuf,
    destination: std::path::PathBuf,
    /// Retained for the post-install replay. Owned rather than borrowed so a
    /// staged set can outlive the scopes that produced each member's bytes.
    bytes: Vec<u8>,
}

/// Write `bytes` beside `path` and replay them, without making them visible
/// at `path`.
pub(crate) fn stage_exact_bytes(
    path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<StagedProduct, String> {
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
    Ok(StagedProduct {
        staged,
        destination: path.to_path_buf(),
        bytes: bytes.to_vec(),
    })
}

fn replay_installed_product(product: &StagedProduct) -> Result<(), String> {
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
pub(crate) fn install_staged_products(products: &[StagedProduct]) -> Result<(), String> {
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

#[cfg(unix)]
pub(crate) fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
pub(crate) fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

/// Remove one companion this publication is not refreshing. The publication
/// contract forbids associating a stale sidecar with newly written bytes, so
/// an unrequested artifact/`.proof` pair cannot survive beside the artifact
/// it no longer commits to. Absence is the expected common case; every other
/// removal failure aborts publication before any new bytes are installed.
pub(crate) fn remove_stale_companion(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale proof companion {}: {error}",
            path.display()
        )),
    }
}

/// The adjacent `.proof`/`.psi` companion path beside `path`, formed by
/// appending `suffix` to the complete artifact filename per the contract.
pub(crate) fn appended_file_name_path(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .expect("a publication path always has a file name")
        .to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}
