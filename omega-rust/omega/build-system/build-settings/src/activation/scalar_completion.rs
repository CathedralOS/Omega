//! Record published-program scalar work in the source compilation's accounting.
use backend_plan::TerminalTargetProviderProposal;
use std::sync::Arc;

pub fn complete_native(
    retained: &products::RetainedTerminalArtifact,
    accounting: &mut source_compiler::evaluation::SourceCompletionAccounting,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    published_program::scalar_completion::complete_native(retained, accounting)
}

pub fn complete_native_demand(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &products::TerminalNativeRealizationProposal,
    demand: Option<Vec<semantic_vocabulary::ScalarApplicationId>>,
    accounting: &mut source_compiler::evaluation::SourceCompletionAccounting,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    published_program::scalar_completion::complete_native_demand(
        artifact, proposal, demand, accounting,
    )
}

pub fn complete<'scope>(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut source_compiler::evaluation::SourceCompletionAccounting,
) -> Result<terminal_installation::AdmittedScalarApplications, String> {
    published_program::scalar_completion::complete(artifact, primary, demand, proposals, accounting)
}

pub fn prepare<'scope>(
    artifact: &Arc<terminal_codec::CanonicalTerminalArtifact>,
    primary: impl Into<Option<&'scope str>>,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    proposals: &[TerminalTargetProviderProposal],
    accounting: &mut source_compiler::evaluation::SourceCompletionAccounting,
) -> Result<products::ProductScalarCompletion, String> {
    published_program::scalar_completion::prepare(artifact, primary, demand, proposals, accounting)
}

pub fn extend_native(
    retained: &products::RetainedTerminalArtifact,
    completion: products::ProductScalarCompletion,
    accounting: &mut source_compiler::evaluation::SourceCompletionAccounting,
) -> Result<Option<terminal_installation::AdmittedScalarApplications>, String> {
    published_program::scalar_completion::extend_native(retained, completion, accounting)
}
