use psi_diagnostics::Diagnostic;

pub(crate) fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}

pub(crate) fn selected_physical_pipeline_not_publishable(
    selections: &omega_optimization_core::OptimizationSelections,
    physical: &omega_optimization_pipeline::StagedOptimizedVerifiedPhysicalPipeline,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` completed the complete verified optimizer pipeline through selected/physical validation (selection identity {:?}) but cannot yet enter native production: the continuation does not cover baseline frame/exit, executable-image, and publication validation; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
        physical.selections(),
    ))]
}

pub(crate) fn selected_physical_pipeline_failed(
    selections: &omega_optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
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
