use boundary_applications::{
    TerminalBoundaryApplicationCoverage, TerminalBoundaryApplicationDemands,
    TerminalBoundaryApplicationRealizations,
};
use diagnostics::Diagnostic;

use super::diagnostics::realization_error;

pub(super) fn retain_boundary_application_coverage(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_scope: Option<&checked_trees_to_terminal_psi::CheckedBoundaryOperatorApplicationScope>,
    supplied: Option<&TerminalBoundaryApplicationCoverage>,
) -> Result<Option<TerminalBoundaryApplicationCoverage>, Vec<Diagnostic>> {
    let Some(checked_scope) = checked_scope else {
        if supplied.is_some() {
            return Err(realization_error(
                "boundary-application custody",
                "D29 coverage requires the exact checked source-to-Terminal scope",
            ));
        }
        return Ok(None);
    };
    let coverage = match supplied {
        Some(coverage) => coverage.clone(),
        None if checked_scope.is_empty() => exact_empty_coverage(artifact)?,
        None => {
            return Err(realization_error(
                "boundary-application custody",
                "a nonempty checked D29 scope requires exact compiler-produced coverage",
            ));
        }
    };
    coverage
        .validate_for_terminal(artifact.manifest().semantic())
        .map_err(|error| realization_error("boundary-application custody", error))?;
    if coverage.references().len() != checked_scope.occurrences().len()
        || !coverage
            .references()
            .iter()
            .zip(checked_scope.occurrences())
            .all(|(reference, occurrence)| {
                reference.terminal_operation() == occurrence.terminal_operation()
            })
    {
        return Err(realization_error(
            "boundary-application custody",
            "source-free D29 coverage differs from exact checked occurrence custody",
        ));
    }
    Ok(Some(coverage))
}

fn exact_empty_coverage(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> Result<TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
    let demands =
        TerminalBoundaryApplicationDemands::new(artifact.manifest().semantic(), Vec::new())
            .map_err(|error| realization_error("boundary-application custody", error))?;
    let realizations = TerminalBoundaryApplicationRealizations::new(&demands, Vec::new())
        .map_err(|error| realization_error("boundary-application custody", error))?;
    TerminalBoundaryApplicationCoverage::new(demands, realizations)
        .map_err(|error| realization_error("boundary-application custody", error))
}
