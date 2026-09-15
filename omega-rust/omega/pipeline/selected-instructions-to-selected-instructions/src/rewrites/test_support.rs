//! Shared source-to-rewrite fixtures for cross-stage allocation controls.

pub use crate::pressure_rematerialization::tests::{
    multiple_use::exercise_multiple_use_rematerialization,
    sole_use::exercise_single_use_rematerialization,
};

/// Retain a checked run produced with an explicit test register budget.
pub fn retain_selected_lowering_run(
    run: crate::StagedSelectedLoweringOptimizationRun,
) -> Result<crate::SelectedInstructionOptimizationOutput, crate::SelectedInstructionOptimizationError>
{
    crate::SelectedInstructionOptimizationOutput::from_evidence(
        crate::SelectedInstructionOptimizationEvidence::LiteralFolds(run),
    )
}
