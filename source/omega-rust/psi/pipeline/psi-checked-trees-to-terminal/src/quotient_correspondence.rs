//! Proof-only quotient correspondence retention at the checked-to-Terminal boundary.
//!
//! This bridge carries the all-or-nothing, source-handle-free direct-`define`
//! batch into semantic-module identity. It does not emit or authorize an
//! executable quotient operation.

use psi_terminal::{TerminalModule, retain_non_executable_quotient_correspondence};
use psi_validation::NonExecutableQuotientCorrespondenceBatch;

use crate::LoweringError;

/// Install a complete proof-only direct-`define` batch derived by semantic
/// validation.
///
/// This entry point is separate from executable quotient admission. It is
/// public so producer tests and later checked admission code can exercise the
/// source-free carrier while ordinary checked validation continues to reject
/// quotient operations. The opaque batch can only be constructed by the
/// all-or-nothing semantic extractor; raw typed-tree vocabulary does not cross
/// into this Terminal producer.
pub fn install_non_executable_quotient_correspondences(
    batch: NonExecutableQuotientCorrespondenceBatch,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let mut retained = batch
        .into_correspondences()
        .into_iter()
        .map(retain_non_executable_quotient_correspondence)
        .collect::<Vec<_>>();
    retained.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut candidate = module.clone();
    candidate.quotient_correspondences = retained;
    psi_terminal_verifier::validate_module_representation(&candidate)
        .map_err(LoweringError::InvalidTerminalModule)?;
    module.quotient_correspondences = candidate.quotient_correspondences;
    Ok(())
}
