// Native compilation receives completed scalar values. Keep the values needed
// by the selected work and refuse a request that omits them.

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
