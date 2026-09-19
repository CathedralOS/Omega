use super::model::{
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationPressure,
};
use crate::PressureRematerializationPolicy;

fn other_rematerialization_policy(
    policy: PressureRematerializationPolicy,
) -> PressureRematerializationPolicy {
    match policy {
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => {
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
        }
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => {
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1
        }
    }
}

fn alternate_budget() -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(7, 7, 7, 7, 7).expect("nonzero budget axes")
}

/// One substitutable field of [`StagedOptimizedActiveResidentRematerializationCustodyReceipt`](super::StagedOptimizedActiveResidentRematerializationCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone.
///
/// `choice_policy` and `classification_policy` are deliberately absent from
/// this enum: `SpillChoicePolicy` and `RecoveryClassificationPolicy` each
/// declare exactly one variant, so no foreign in-vocabulary value exists to
/// substitute. Those fields are named closed by the policy vocabulary; the
/// mutation-matrix test pins their exhaustiveness with a compile-time match
/// so any new variant forces this matrix to grow. The independent checker is
/// `validate_optimized_active_resident_rematerialization`, and joined
/// `replay_allocation` surfaces its rejection as
/// `AllocationReplayError::ActiveResidentRematerialization`.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedActiveResidentRematerializationCustodyFieldForTest {
    Source,
    Choices,
    ChoiceUsage,
    Classifications,
    ClassificationUsage,
    Rematerialization,
    RematerializationPolicy,
    RematerializationUsage,
    Budget,
    TransformedSelected,
    Liveness,
    Ranges,
    Legality,
    Homes,
    Manifest,
    FunctionCount,
    VirtualRegisterCount,
    AppliedCount,
    RewrittenUseCount,
    AssignmentCount,
}

impl StagedOptimizedActiveResidentRematerialization {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedActiveResidentRematerializationCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Choices => {
                self.custody.choices = crate::SpillChoiceIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::ChoiceUsage => {
                self.custody.choice_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Classifications => {
                self.custody.classifications =
                    crate::RecoveryClassificationIdentity::from_bytes([0xb3; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::ClassificationUsage => {
                self.custody.classification_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Rematerialization => {
                self.custody.rematerialization =
                    crate::PressureRematerializationIdentity::from_bytes([0xb4; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::RematerializationPolicy => {
                self.custody.rematerialization_policy =
                    other_rematerialization_policy(self.custody.rematerialization_policy);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::RematerializationUsage => {
                self.custody.rematerialization_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Budget => {
                self.custody.budget = alternate_budget();
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::TransformedSelected => {
                self.custody.transformed_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Liveness => {
                self.custody.liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Ranges => {
                self.custody.ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Legality => {
                self.custody.legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Homes => {
                self.custody.homes = crate::RegisterHomeIdentity::from_bytes([0xb5; 32]);
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::Manifest => {
                self.custody.manifest =
                    optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes(
                        [0xb6; 32],
                    );
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::VirtualRegisterCount => {
                self.custody.virtual_register_count += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::AppliedCount => {
                self.custody.applied_count += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::RewrittenUseCount => {
                self.custody.rewritten_use_count += 1;
            }
            OptimizedActiveResidentRematerializationCustodyFieldForTest::AssignmentCount => {
                self.custody.assignment_count += 1;
            }
        }
    }
}

/// One substitutable field of [`StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt`](super::StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt).
/// The custody matrix substitutes exactly one field per leg so a rejection
/// attributes to that claim alone.
///
/// `choice_policy` and `classification_policy` are deliberately absent from
/// this enum: `SpillChoicePolicy` and `RecoveryClassificationPolicy` each
/// declare exactly one variant, so no foreign in-vocabulary value exists to
/// substitute. Those fields are named closed by the policy vocabulary; the
/// mutation-matrix test pins their exhaustiveness with a compile-time match
/// so any new variant forces this matrix to grow. The independent checker is
/// `validate_optimized_active_resident_rematerialization_pressure`, and
/// joined `replay_allocation` surfaces its rejection as
/// `AllocationReplayError::ActiveResidentRematerialization`.
#[derive(Debug, Clone, Copy)]
pub enum OptimizedActiveResidentRematerializationPressureCustodyFieldForTest {
    Source,
    Choices,
    ChoiceUsage,
    Classifications,
    ClassificationUsage,
    Rematerialization,
    RematerializationPolicy,
    RematerializationUsage,
    Budget,
    TransformedSelected,
    Liveness,
    Ranges,
    Legality,
    FunctionCount,
    VirtualRegisterCount,
    AppliedCount,
    RewrittenUseCount,
}

impl StagedOptimizedActiveResidentRematerializationPressure {
    /// Mutate only retained receipt facts; the donor's nested source receipt
    /// is authentic foreign evidence and grants no new authority here.
    pub fn corrupt_custody_for_test(
        &mut self,
        field: OptimizedActiveResidentRematerializationPressureCustodyFieldForTest,
        donor: &Self,
    ) {
        match field {
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Source => {
                self.custody.source = donor.custody.source;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Choices => {
                self.custody.choices = crate::SpillChoiceIdentity::from_bytes([0xb2; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::ChoiceUsage => {
                self.custody.choice_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Classifications => {
                self.custody.classifications =
                    crate::RecoveryClassificationIdentity::from_bytes([0xb3; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::ClassificationUsage => {
                self.custody.classification_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Rematerialization => {
                self.custody.rematerialization =
                    crate::PressureRematerializationIdentity::from_bytes([0xb4; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::RematerializationPolicy => {
                self.custody.rematerialization_policy =
                    other_rematerialization_policy(self.custody.rematerialization_policy);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::RematerializationUsage => {
                self.custody.rematerialization_usage.iterations += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Budget => {
                self.custody.budget = alternate_budget();
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::TransformedSelected => {
                self.custody.transformed_selected =
                    selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0xa9; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Liveness => {
                self.custody.liveness =
                    selected_instructions::LivenessIdentity::from_bytes([0xaa; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Ranges => {
                self.custody.ranges =
                    selected_instructions::LiveRangeIdentity::from_bytes([0xab; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::Legality => {
                self.custody.legality =
                    crate::AllocationLegalityIdentity::from_bytes([0xb1; 32]);
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::FunctionCount => {
                self.custody.function_count += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::VirtualRegisterCount => {
                self.custody.virtual_register_count += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::AppliedCount => {
                self.custody.applied_count += 1;
            }
            OptimizedActiveResidentRematerializationPressureCustodyFieldForTest::RewrittenUseCount => {
                self.custody.rewritten_use_count += 1;
            }
        }
    }
}
