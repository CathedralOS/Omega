//! Prepare proof companions before installing any artifact bytes.
use crate::PccPublicationReceipt;
use platform_custody::file_publication::{StagedFile, stage_exact_bytes};
use std::path::{Path, PathBuf};
use terminal_codec::PreparedPsiProofPair;

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

/// Describe the selected destination; retain this receipt only after the staged
/// file set or package tree has installed successfully.
pub(crate) fn psi_pair_receipt(
    pair: &PreparedPsiProofPair,
    artifact_path: &Path,
) -> PccPublicationReceipt {
    PccPublicationReceipt {
        product: terminal_codec::PccProductKind::Psi,
        artifact_path: artifact_path.to_path_buf(),
        artifact_byte_len: pair.artifact_bytes().len() as u64,
        sidecar_path: appended_file_name_path(artifact_path, ".proof"),
        sidecar_byte_len: pair.sidecar_bytes().len() as u64,
    }
}

/// Append both staged members without installing either destination. If the
/// second staging fails, the previously published pair remains untouched.
pub(crate) fn stage_psi_pair(
    pair: &PreparedPsiProofPair,
    artifact_path: &Path,
    staged_products: &mut Vec<StagedFile>,
) -> Result<PccPublicationReceipt, String> {
    let receipt = psi_pair_receipt(pair, artifact_path);
    staged_products.push(stage_exact_bytes(
        artifact_path,
        pair.artifact_bytes(),
        false,
    )?);
    staged_products.push(stage_exact_bytes(
        &receipt.sidecar_path,
        pair.sidecar_bytes(),
        false,
    )?);
    Ok(receipt)
}
