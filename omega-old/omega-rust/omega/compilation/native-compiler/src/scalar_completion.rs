// The scalar values native compilation reads arrive complete in its request
// (`build_settings::scalar_completion::complete_native`), so the native
// pipeline never pauses for evaluation. This keeps them only where the selected
// work needs them and refuses work that needs them when they are missing.

use diagnostics::Diagnostic;
use terminal_installation::AdmittedScalarApplications;

pub(crate) fn required_scalars<'input>(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    demand: &[semantic_vocabulary::ScalarApplicationId],
    supplied: Option<&'input AdmittedScalarApplications>,
) -> Result<Option<&'input AdmittedScalarApplications>, Vec<Diagnostic>> {
    if !verified_optimization_input::requires_scalar_values(artifact, demand) {
        return Ok(None);
    }
    supplied.map(Some).ok_or_else(|| {
        vec![Diagnostic::error(
            "native compilation needs the scalar values its program reads, completed before it starts",
        )]
    })
}
