//! Stage codec-prepared Psi pairs at the selected product destination.
//!
//! The codec owns serialization and receiver replay. This adapter describes
//! output paths and stages both members before the caller begins installation.

use std::path::Path;

use super::appended_file_name_path;
use crate::report::PccPublicationReceipt;
use platform_custody::file_publication::{StagedFile, stage_exact_bytes};
use terminal_codec::PreparedPsiProofPair;

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
