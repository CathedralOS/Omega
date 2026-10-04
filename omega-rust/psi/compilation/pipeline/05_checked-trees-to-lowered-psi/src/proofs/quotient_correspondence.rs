//! Proof-only quotient correspondence retention at the checked-to-Terminal boundary.
//!
//! This bridge carries the all-or-nothing, source-handle-free direct-`define`
//! and bounded transport-backed direct-`lift` batch into semantic-module
//! identity. It does not emit or authorize an executable quotient operation.
//!
//! [`retain_checked_quotient_correspondences`] is the production entrance:
//! machine lowering calls it once the selected module is complete and before
//! module validation. A program without a `Quotient::define`/`Quotient::lift`
//! request leaves the module untouched. A program with requests either
//! installs the complete batch the semantic extractor admits, or fails closed
//! with [`LoweringError::UnadmittedQuotientRequest`] naming every extraction
//! diagnostic — no partial table and no silently dropped request. The
//! installed table still owns no executable operation: module validation
//! afterwards rejects a nonempty table with
//! `ModuleError::NonExecutableQuotientCorrespondence` until executable
//! quotient lowering exists
//! ([published quotient correspondence](../../../../../../wiki/spec/proofs/quotients.md#published-quotient-correspondence)).
//!
//! Boundary: ordinary checked validation (`validation::validate_program`,
//! `reject_quotient_operation_requests`) still rejects every request before
//! checked trees exist, and the checked stage exits every value path whose
//! call carries `quotient_operation`, so on the current route the production
//! entrance sees no request. Its tests substitute a request-bearing typed
//! program into a checked baseline, the same input the extractor reads, and
//! answer termination from `facts.termination` as the compiler route does.

use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionNode;
use terminal_psi::{TerminalModule, retain_non_executable_quotient_correspondence};
use validation::NonExecutableQuotientCorrespondenceBatch;

use crate::lowering_error::LoweringError;

/// Install a complete proof-only quotient-correspondence batch derived by
/// semantic validation.
///
/// This entry point is separate from executable quotient admission:
/// [`retain_checked_quotient_correspondences`] is its production caller and
/// producer tests exercise the source-free carrier directly. The opaque batch
/// can only be constructed by the all-or-nothing semantic extractor; raw
/// typed-tree vocabulary does not cross into this Terminal producer.
pub(crate) fn install_non_executable_quotient_correspondences(
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

/// Retain the checked program's quotient requests on the selected module.
///
/// The semantic extractor rederives every request from the checked stage's
/// retained program input (`CheckedTrees::typed`) rather than trusting any
/// checked-stage summary, except termination eligibility: the typed machine
/// carries no guarantee on the compiler route (the checked stage proves
/// termination after validation into `facts.termination`), so that one
/// judgment reads the checked termination facts. With no request present the
/// module is returned untouched and the extractor never runs.
pub(crate) fn retain_checked_quotient_correspondences(
    checked: &CheckedTrees,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    if !program_carries_quotient_request(checked) {
        return Ok(());
    }
    let checked_termination = |machine: symbols::SymbolHandle| {
        checked
            .facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    };
    let batch = validation::extract_non_executable_quotient_correspondences_with_termination(
        &checked.typed,
        &checked_termination,
    )
    .map_err(|diagnostics| LoweringError::UnadmittedQuotientRequest {
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
    })?;
    install_non_executable_quotient_correspondences(batch, module)
}

fn program_carries_quotient_request(checked: &CheckedTrees) -> bool {
    checked
        .typed
        .expression_table
        .iter_expressions()
        .any(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.quotient_operation.is_some())
        })
}
