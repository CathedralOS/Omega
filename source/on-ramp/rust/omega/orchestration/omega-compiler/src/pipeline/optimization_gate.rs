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
        "selected optimization{} `{names}` require{} the complete verified optimizer pipeline, which is not available yet; no output was installed",
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

/// The clean selected lane may validate optimization and retain its custody
/// through selected lowering, strict register homes, and post-allocation
/// machine validation, but cannot yet create a deployable component until
/// frame/exit, emission, artifact, and optimized publication validation bind
/// the ledger to every physical realization record.
pub(super) fn optimized_publication_unavailable(
    selections: &OptimizationSelections,
) -> Vec<Diagnostic> {
    debug_assert!(!selections.is_empty());
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` completed the verified physical pipeline through post-allocation machine validation, but frame/exit, emission, artifact, and optimized component publication validation are not available yet; no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
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
            Optimization::ControlFlowCleanup,
            Optimization::X86RelaxConditionalBranchesToRel8V1,
        ])
        .expect("unique selections");
        let diagnostics = require_available_pipeline(&selections)
            .expect_err("unimplemented optimizer must fail closed");
        assert_eq!(diagnostics.len(), 1);
        let message = diagnostics[0].message.as_str();
        assert!(message.contains("`ControlFlowCleanup`, `X86RelaxConditionalBranchesToRel8V1`"));
        assert!(message.contains("no output was installed"));
    }
}
