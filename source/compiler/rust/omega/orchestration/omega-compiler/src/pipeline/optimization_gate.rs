use omega_optimization_core::OptimizationSelections;
use psi_diagnostics::Diagnostic;

/// Fail closed until the verified Terminal-Psi-derived optimizer pipeline can
/// publish a complete validated realization for every selected family.
///
/// The empty branch deliberately does no work and constructs no registry,
/// analysis manager, policy, cost model, or report.
pub(super) fn require_available_pipeline(
    selections: &OptimizationSelections,
) -> Result<(), Vec<Diagnostic>> {
    if selections.is_empty() {
        return Ok(());
    }
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    Err(vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` require{} the verified Terminal-Psi optimizer pipeline, which is not available yet; no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
        if selections.as_slice().len() == 1 {
            "s"
        } else {
            ""
        },
    ))])
}

#[cfg(test)]
mod tests {
    use super::require_available_pipeline;
    use omega_optimization_core::{Optimization, OptimizationSelections};

    #[test]
    fn empty_selection_constructs_nothing_and_passes() {
        require_available_pipeline(&OptimizationSelections::default())
            .expect("empty selection remains the ordinary pipeline");
    }

    #[test]
    fn selected_pipeline_fails_once_with_canonical_names() {
        let selections = OptimizationSelections::new([
            Optimization::ProofCheckElision,
            Optimization::ControlFlowCleanup,
        ])
        .expect("unique selections");
        let diagnostics = require_available_pipeline(&selections)
            .expect_err("unimplemented optimizer must fail closed");
        assert_eq!(diagnostics.len(), 1);
        let message = diagnostics[0].message.as_str();
        assert!(message.contains("`ControlFlowCleanup`, `ProofCheckElision`"));
        assert!(message.contains("no output was installed"));
    }
}
