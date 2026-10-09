use super::super::super::super::super::{analyze_live_ranges, analyze_liveness};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
    RecoveryClassificationIdentity, SpillChoiceIdentity,
};
use selected_instructions::{LiveRangeIdentity, LiveRangePoint};

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::TargetRegisterEnvironmentIdentity;
use selected_instructions::{
    SelectedInstructionId, SelectedInstructionPlanIdentity, SelectedTerminator, VirtualRegisterId,
    selected_instruction_plan_identity,
};
use semantic_vocabulary::ValueId;

use super::super::compute::build_functions;
use super::super::{
    PressureRematerializationError, PressureRematerializationPlan, PressureRematerializationPolicy,
    PressureRematerializationValidationReceipt, ValidatedPressureRematerialization,
    pressure_rematerialization_identity,
};
