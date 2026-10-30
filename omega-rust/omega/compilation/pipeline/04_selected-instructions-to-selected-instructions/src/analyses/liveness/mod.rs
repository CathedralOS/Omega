//! Optimizer module role: executable entrance. Selected-CFG liveness compute; `validate` is the retained-plan verifier.

use crate::ValidatedSelectedAnalysis;
pub(crate) mod compute;
pub(crate) mod edge_values;
pub(crate) mod validate;

use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{LivenessIdentity, LivenessPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

/// Compute bounded selected-CFG liveness facts. The result grants no
/// interval, allocation, emission, or publication authority.
pub fn analyze_liveness<S: ValidatedSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    Ok(validate::admit_validated_liveness(plan, &[], &[]))
}

/// Reuse unchanged function computations from `previous_liveness`; the new
/// receipt still covers the whole plan.
pub fn analyze_liveness_reusing(
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<ValidatedLiveness, LivenessError> {
    let (plan, carried) =
        compute::compute_terminal_liveness_reusing(previous, previous_liveness, selected)?;
    Ok(validate::admit_validated_liveness(
        plan,
        &carried,
        &previous_liveness.function_digests,
    ))
}

mod staging;
pub(crate) use staging::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
    stage_optimized_liveness,
};

copy_receipt! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LivenessValidationReceipt {
        pub(crate) identity: LivenessIdentity,
        pub(crate) selected: SelectedInstructionPlanIdentity,
        pub(crate) optimization_unit: OptimizationUnitIdentity,
        pub(crate) fuel_schedule: FuelScheduleIdentity,
    }
}

shared_plan_carrier!(ValidatedLiveness: LivenessPlan, LivenessValidationReceipt {});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivenessError {
    RootMismatch,
    UnsupportedUseDef {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedTiedOperand {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    UnsupportedEarlyClobber {
        function: usize,
        instruction: u32,
        operand: u16,
    },
    FunctionMismatch {
        function: usize,
    },
    BlockMismatch {
        function: usize,
        block: u32,
    },
    InstructionMismatch {
        function: usize,
        instruction: u32,
    },
    SuccessorMismatch {
        function: usize,
        block: u32,
        ordinal: u8,
    },
    NonCanonicalSet {
        function: usize,
        instruction: Option<u32>,
    },
    NonDensePositions {
        function: usize,
    },
    TransferMismatch {
        function: usize,
        instruction: u32,
    },
    FixedConstraintMismatch {
        function: usize,
    },
}

failed_display!(LivenessError, "Terminal selected liveness failed");
