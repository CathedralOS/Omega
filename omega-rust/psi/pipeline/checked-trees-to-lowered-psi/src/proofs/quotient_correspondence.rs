//! Proof-only quotient correspondence retention at the checked-to-Terminal boundary.
//!
//! This bridge carries the all-or-nothing, source-handle-free direct-`define`
//! and bounded transport-backed direct-`lift` batch into semantic-module
//! identity. It does not emit or authorize an executable quotient operation.

#[cfg(test)]
use terminal_psi::{TerminalModule, retain_non_executable_quotient_correspondence};
#[cfg(test)]
use validation::NonExecutableQuotientCorrespondenceBatch;

#[cfg(test)]
use crate::lowering_error::LoweringError;

/// Install a complete proof-only quotient-correspondence batch derived by
/// semantic validation.
///
/// This entry point is separate from executable quotient admission and is a
/// test-only route: producer tests exercise the source-free carrier while
/// ordinary checked validation continues to reject quotient operations, and
/// checked admission code can un-gate it when it exists. The opaque batch can
/// only be constructed by the all-or-nothing semantic extractor; raw
/// typed-tree vocabulary does not cross into this Terminal producer.
#[cfg(test)]
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
    terminal_verifier::validate_module_representation(&candidate)
        .map_err(LoweringError::InvalidTerminalModule)?;
    module.quotient_correspondences = candidate.quotient_correspondences;
    Ok(())
}
