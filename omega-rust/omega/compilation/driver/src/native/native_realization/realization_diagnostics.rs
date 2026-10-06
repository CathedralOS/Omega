//! Diagnostic constructors shared by every realization stage.

use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::ArtifactLoweringError;

pub(crate) fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
    match error {
        ArtifactLoweringError::CanonicalArtifact(error) => {
            realization_error("canonical artifact replay", error)
        }
        ArtifactLoweringError::SemanticDecode(error) => {
            realization_error("semantic section", error)
        }
        error => realization_error("native artifact lowering", error),
    }
}

pub(crate) fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}

pub(crate) fn selected_physical_pipeline_failed(
    selections: &optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    if selections.is_empty() {
        return realization_error("identity physical pipeline", error);
    }
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` failed in the complete verified optimizer pipeline during selected/physical validation: {error}; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
}
