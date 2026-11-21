//! Complete the scalar applications read by the target execution selected in stage 01.
use diagnostics::Diagnostic;
use native_scalar_inputs::EvaluationReceipt;
use psi_scalar_applications::AdmittedScalarApplications;
use verified_optimization_input::TerminalNativeRealizationProposal;

pub(crate) fn complete(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &TerminalNativeRealizationProposal,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    receipts: &mut Vec<EvaluationReceipt>,
) -> Result<Option<AdmittedScalarApplications>, Vec<Diagnostic>> {
    // Scalar conditions must be discharged even when no reachable code reads a scalar.
    let conditions = &artifact
        .semantic_module()
        .scalars
        .scalar_applications
        .conditions;
    if demand.is_empty() && conditions.is_empty() {
        return Ok(None);
    }
    native_scalar_inputs::complete(
        artifact,
        proposal.target_profile().target_name(),
        demand,
        proposal.target_provider_proposals(),
        receipts,
    )
    .map(Some)
    .map_err(|reason| vec![Diagnostic::error(format!("constant checking: {reason}"))])
}
