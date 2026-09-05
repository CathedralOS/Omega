//! Ordered literal-fold and selected-lowering completion records.
//!
//! These records do not grant validation or publication authority. The owning
//! transform independently reconstructs and compares them before admission.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedLoweringOptimizationCustodyReceipt {
    pub identity: SelectedLoweringOptimizationCompletionIdentity,
    pub source: AllocationLegalityCustodyReceipt,
    pub selections: OptimizationSelectionIdentity,
    pub selected_lowering_selections: OptimizationSelectionIdentity,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub iteration_bound: usize,
    pub action_count: usize,
    pub initial_virtual_register_count: usize,
    pub iterations: Vec<LiteralFoldIterationReceipt>,
    pub attempt: LiteralFoldAttemptReceipt,
    pub final_selected: SelectedInstructionPlanIdentity,
    pub final_liveness: crate::LivenessIdentity,
    pub final_ranges: crate::LiveRangeIdentity,
    pub final_legality: crate::AllocationLegalityIdentity,
    pub final_virtual_register_count: usize,
}

impl SelectedLoweringOptimizationCustodyReceipt {
    pub const fn identity(&self) -> SelectedLoweringOptimizationCompletionIdentity {
        self.identity
    }
    pub const fn source(&self) -> AllocationLegalityCustodyReceipt {
        self.source
    }
    pub const fn selections(&self) -> OptimizationSelectionIdentity {
        self.selections
    }
    pub const fn selected_lowering_selections(&self) -> OptimizationSelectionIdentity {
        self.selected_lowering_selections
    }
    pub const fn budget(&self) -> OptimizationWorkBudget {
        self.budget
    }
    pub const fn usage(&self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn iteration_bound(&self) -> usize {
        self.iteration_bound
    }
    pub const fn action_count(&self) -> usize {
        self.action_count
    }
    pub const fn initial_virtual_register_count(&self) -> usize {
        self.initial_virtual_register_count
    }
    pub fn iterations(&self) -> &[LiteralFoldIterationReceipt] {
        &self.iterations
    }
    pub const fn attempt(&self) -> LiteralFoldAttemptReceipt {
        self.attempt
    }
    pub const fn final_selected(&self) -> SelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> crate::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> crate::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> crate::AllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAttemptReceipt {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub source_ranges: crate::LiveRangeIdentity,
    pub source_legality: crate::AllocationLegalityIdentity,
    pub choices: crate::SpillChoiceIdentity,
    pub choice_policy: SpillChoicePolicy,
    pub choice_usage: OptimizationWorkUsage,
    pub recovery: crate::RecoveryClassificationIdentity,
    pub recovery_policy: RecoveryClassificationPolicy,
    pub recovery_usage: OptimizationWorkUsage,
    pub fold: LiteralFoldIdentity,
    pub fold_policy: LiteralFoldPolicy,
    pub fold_usage: OptimizationWorkUsage,
    pub applied_count: usize,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl LiteralFoldAttemptReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> crate::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> crate::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> crate::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> crate::RecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> RecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> LiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> LiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldCustodyReceipt {
    pub source: AllocationLegalityCustodyReceipt,
    pub iterations: Vec<LiteralFoldIterationReceipt>,
    pub transformations: Vec<LiteralFoldIdentity>,
    pub final_selected: SelectedInstructionPlanIdentity,
    pub final_liveness: crate::LivenessIdentity,
    pub final_ranges: crate::LiveRangeIdentity,
    pub final_legality: crate::AllocationLegalityIdentity,
    pub final_virtual_register_count: usize,
    pub final_entry_transition_count: usize,
}

impl LiteralFoldCustodyReceipt {
    pub const fn source(&self) -> AllocationLegalityCustodyReceipt {
        self.source
    }
    pub fn iterations(&self) -> &[LiteralFoldIterationReceipt] {
        &self.iterations
    }
    pub fn transformations(&self) -> &[LiteralFoldIdentity] {
        &self.transformations
    }
    pub const fn final_selected(&self) -> SelectedInstructionPlanIdentity {
        self.final_selected
    }
    pub const fn final_liveness(&self) -> crate::LivenessIdentity {
        self.final_liveness
    }
    pub const fn final_ranges(&self) -> crate::LiveRangeIdentity {
        self.final_ranges
    }
    pub const fn final_legality(&self) -> crate::AllocationLegalityIdentity {
        self.final_legality
    }
    pub const fn final_virtual_register_count(&self) -> usize {
        self.final_virtual_register_count
    }
    pub const fn final_entry_transition_count(&self) -> usize {
        self.final_entry_transition_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldIterationReceipt {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub source_ranges: crate::LiveRangeIdentity,
    pub source_legality: crate::AllocationLegalityIdentity,
    pub choices: crate::SpillChoiceIdentity,
    pub choice_policy: SpillChoicePolicy,
    pub choice_usage: OptimizationWorkUsage,
    pub recovery: crate::RecoveryClassificationIdentity,
    pub recovery_policy: RecoveryClassificationPolicy,
    pub recovery_usage: OptimizationWorkUsage,
    pub fold: LiteralFoldIdentity,
    pub fold_policy: LiteralFoldPolicy,
    pub fold_usage: OptimizationWorkUsage,
    pub transformed_selected: SelectedInstructionPlanIdentity,
    pub fresh_liveness: crate::LivenessIdentity,
    pub fresh_ranges: crate::LiveRangeIdentity,
    pub fresh_legality: crate::AllocationLegalityIdentity,
}

impl LiteralFoldIterationReceipt {
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_ranges(self) -> crate::LiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> crate::AllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn choices(self) -> crate::SpillChoiceIdentity {
        self.choices
    }
    pub const fn choice_policy(self) -> SpillChoicePolicy {
        self.choice_policy
    }
    pub const fn choice_usage(self) -> OptimizationWorkUsage {
        self.choice_usage
    }
    pub const fn recovery(self) -> crate::RecoveryClassificationIdentity {
        self.recovery
    }
    pub const fn recovery_policy(self) -> RecoveryClassificationPolicy {
        self.recovery_policy
    }
    pub const fn recovery_usage(self) -> OptimizationWorkUsage {
        self.recovery_usage
    }
    pub const fn fold(self) -> LiteralFoldIdentity {
        self.fold
    }
    pub const fn fold_policy(self) -> LiteralFoldPolicy {
        self.fold_policy
    }
    pub const fn fold_usage(self) -> OptimizationWorkUsage {
        self.fold_usage
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn fresh_liveness(self) -> crate::LivenessIdentity {
        self.fresh_liveness
    }
    pub const fn fresh_ranges(self) -> crate::LiveRangeIdentity {
        self.fresh_ranges
    }
    pub const fn fresh_legality(self) -> crate::AllocationLegalityIdentity {
        self.fresh_legality
    }
}
