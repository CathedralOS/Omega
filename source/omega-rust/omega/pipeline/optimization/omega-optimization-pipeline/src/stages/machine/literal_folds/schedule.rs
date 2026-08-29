use omega_regalloc::LiteralFoldPolicy;

use super::SelectedLoweringOptimizationSchedule;

/// Convert the rule-owned policy into this custody stage's execution receipt.
pub(super) const fn selected_lowering_schedule(
    policy: LiteralFoldPolicy,
) -> SelectedLoweringOptimizationSchedule {
    match policy {
        LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1 => {
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddImmediateToNoChangeV1
        }
        LiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1 => {
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactSubtractImmediateToNoChangeV1
        }
        LiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1 => {
            SelectedLoweringOptimizationSchedule::SelectedIncomingU12ExactAddAndSubtractImmediateToNoChangeV1
        }
    }
}
