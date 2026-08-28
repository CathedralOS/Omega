//! Proof-only quotient correspondence retention at the checked-to-Terminal boundary.
//!
//! This bridge carries the all-or-nothing, source-handle-free direct-`define`
//! batch into semantic-module identity. It does not emit or authorize an
//! executable quotient operation.

use psi_terminal::{TerminalModule, retain_non_executable_quotient_correspondence};
use psi_typed_trees::TypedTrees;

use crate::LoweringError;

/// Install the complete proof-only direct-`define` batch derived from the
/// typed program.
///
/// This entry point is separate from executable quotient admission. It is
/// public so producer tests and later checked admission code can exercise the
/// carrier while ordinary checked validation continues to reject quotient
/// operations. Accepting `TypedTrees` is deliberate: this function does not
/// claim that ordinary checked validation has admitted the request.
pub fn install_non_executable_quotient_correspondences(
    program: &TypedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    let certificates = psi_validation::extract_non_executable_quotient_correspondences(program)
        .map_err(|diagnostics| {
            LoweringError::InvalidQuotientCorrespondence(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect(),
            )
        })?;
    let mut retained = certificates
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
