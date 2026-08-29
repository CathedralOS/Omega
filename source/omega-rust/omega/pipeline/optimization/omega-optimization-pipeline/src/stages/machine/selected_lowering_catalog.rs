//! Exact selected-lowering rule catalog and supported compositions.

use omega_optimization_core::{Optimization, OptimizationSelections};
use omega_regalloc::LiteralFoldPolicy;

use super::literal_folds::{
    OptimizedLiteralFoldCustodyError, SelectedLoweringOptimizationSchedule,
};

/// Canonical selected-lowering rule order. Combined execution is an explicit
/// composition of these two exact names, not another source-visible suite.
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 2] = [
    Optimization::SelectedIncomingU12ExactAddImmediate,
    Optimization::SelectedIncomingU12ExactSubtractImmediate,
];

pub(super) fn selected_lowering_contract(
    selections: &OptimizationSelections,
) -> Result<
    (SelectedLoweringOptimizationSchedule, LiteralFoldPolicy),
    OptimizedLiteralFoldCustodyError,
> {
    match selections.as_slice() {
        [Optimization::SelectedIncomingU12ExactAddImmediate] => Ok((
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddImmediateToNoChangeV1,
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
        )),
        [Optimization::SelectedIncomingU12ExactSubtractImmediate] => Ok((
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactSubtractImmediateToNoChangeV1,
            LiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1,
        )),
        [
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ] => Ok((
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddAndSubtractImmediateToNoChangeV1,
            LiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1,
        )),
        [] => Err(OptimizedLiteralFoldCustodyError::MissingSelectedLoweringOptimization),
        selections => Err(
            OptimizedLiteralFoldCustodyError::UnsupportedSelectedLoweringOptimization(
                selections[0],
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_optimization_core::OptimizationExecutionPhase;

    #[test]
    fn ordered_catalog_covers_every_declared_selected_lowering_optimization_once() {
        let declared = Optimization::ALL
            .into_iter()
            .filter(|optimization| {
                optimization.execution_phase() == OptimizationExecutionPhase::SelectedLowering
            })
            .collect::<Vec<_>>();
        assert_eq!(declared, ORDERED_SELECTED_LOWERING_RULES);
        for optimization in ORDERED_SELECTED_LOWERING_RULES {
            let selections = OptimizationSelections::new([optimization]).unwrap();
            assert!(selected_lowering_contract(&selections).is_ok());
        }
        let composition = OptimizationSelections::new(ORDERED_SELECTED_LOWERING_RULES).unwrap();
        assert!(selected_lowering_contract(&composition).is_ok());
    }
}
