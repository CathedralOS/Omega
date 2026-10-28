//! Optimizer module role: executable entrance. Exact pressure-recovery eligibility classification entrance.
use crate::ValidatedAllocationLegality;
use crate::ValidatedLiveRanges;
use crate::ValidatedSelectedAnalysis;
use crate::ValidatedSpillChoices;
use register_homes::RecoveryClassificationPolicy;

pub(crate) mod compute;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, SpillChoiceIdentity,
};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{LiveRangeIdentity, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

use crate::WorkDefect;

/// Classify an already selected pressure victim under one exact recovery
/// eligibility policy without selecting or applying a recovery strategy.
pub fn classify_pressure_recovery<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let plan = compute::compute_terminal_recovery_classifications(
        selected,
        ranges,
        legality,
        spill_choices,
        policy,
        budget,
    )?;
    seal_recovery_classifications(plan)
}

// The work accounting this plan was produced under already admitted its exact
// total, so this seal does not repeat that admission.
seal_plan!(
    seal_recovery_classifications(RecoveryClassificationPlan)
        -> ValidatedRecoveryClassifications: RecoveryClassificationValidationReceipt
        = register_homes::recovery_classification_identity, RecoveryClassificationError
    {
        selected,
        spill_choices,
        ranges,
        legality,
        register_environment,
        allocator_availability,
        optimization_unit,
        fuel_schedule,
        policy,
        usage
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryClassificationValidationReceipt {
    pub(crate) identity: RecoveryClassificationIdentity,
    pub(crate) selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: RecoveryClassificationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
}

impl RecoveryClassificationValidationReceipt {
    receipt_accessor_rows!(RecoveryClassificationValidationReceipt {
        identity: RecoveryClassificationIdentity,
        selected: SelectedInstructionPlanIdentity,
        spill_choices: SpillChoiceIdentity,
        policy: RecoveryClassificationPolicy,
        usage: OptimizationWorkUsage,
    });
    shared_analysis_identity_accessors!(pub);
}

validated_plan_carrier!(
    ValidatedRecoveryClassifications: RecoveryClassificationPlan,
    RecoveryClassificationValidationReceipt
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryClassificationError {
    RootMismatch,
    UnsupportedPolicy,
    /// Bounded work accounting refused: the logical receipt overflowed or
    /// exceeded the selected budget.
    Work(WorkDefect),
    FunctionMismatch {
        function: usize,
    },
    ChoiceMismatch {
        function: usize,
    },
    VictimMismatch {
        function: usize,
        register: u32,
    },
    ClassificationMismatch {
        function: usize,
    },
    UsageMismatch,
}

failed_display!(
    RecoveryClassificationError,
    "Terminal recovery classification failed"
);
