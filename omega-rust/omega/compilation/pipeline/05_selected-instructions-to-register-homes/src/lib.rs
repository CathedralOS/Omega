#![forbid(unsafe_code)]

/*
We assign physical register homes and execute admitted register-pressure
recovery over the selected instruction program (`Omega 05`). Our entry point,
`stage_register_allocation` in `register_allocation.rs`, consumes the preceding
`Omega 04` stage's `SelectedOptimizationOutput`, sequences transition-free home
assignment alongside declared or runtime-spill pressure recovery, and publishes
one validated `RetainedAllocation` carrying the current allocated program and
its checked facts.
*/

/*
Register allocation must satisfy target register views, storage/write-unit
aliasing, fixed-operand pins, tied operands, directional early clobbers, and
ABI call reservations without inventing unchecked moves or silently altering
recovery policy. We separate transition-free home assignment (`home_assignment.rs`)
from explicit CFG recovery rewrites (`rewrites.rs`, `fixed_view_copy_recovery.rs`,
`pressure_rematerialization_recovery.rs`, `runtime_spill_and_split.rs`) and seal
`ValidatedSelectedAnalysis` so every post-rewrite graph is re-analyzed through
fresh liveness, live-range, and legality checks before its homes are accepted.
*/

/*
- Input: `SelectedOptimizationOutput` from `Omega 04` (`selected-instructions-to-selected-instructions`).
- Output: `RetainedAllocation` (`output.rs`) and `ValidatedAllocatedCalleeSavedRequirements` (`preservation.rs`) consumed by `Omega 06` (`register-homes-to-machine-plan`).
- Flat module layout:
  - `register_allocation.rs`: stage entry routing identity, completed rewrite runs, declared recovery rules, and runtime spill fallback.
  - `assignment.rs`: baseline, transformed, recovery, runtime-spill, and post-allocation manifest staging.
  - `home_assignment.rs`: transition-free constraint solver, interference/early-clobber preparation, and independent home replay.
  - `rewrites.rs`: fixed-occurrence materialization, fixed-view segment-home staging, settlement-gap alignment, and active-resident rematerialization staging.
  - `fixed_view_copy_recovery.rs`: fixed-view copy materialization and independent CFG replay.
  - `pressure_rematerialization_recovery.rs`: active-resident immediate rematerialization under register pressure.
  - `runtime_spill_and_split.rs`: runtime value spilling, live-range splitting, and runtime rematerialization.
  - `reanalysis.rs`: liveness, live-range, and legality reanalysis over fixed-view copy graphs.
  - `output.rs`: `AllocationOutput` projection and `RetainedAllocation` admission.
  - `preservation.rs`: allocation-visible callee-saved register modification discovery and replay.
*/

macro_rules! receipt_accessor_rows {
    (ref $name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { self.$field } )*
    };
    (borrow $name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { &self.$field } )*
    };
    ($name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(self) -> $ty { self.$field } )*
    };
}

macro_rules! copy_receipt {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($fvis:vis $field:ident : $ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $($fvis $field: $ty),*
        }

        impl $name {
            receipt_accessor_rows!($name { $($field: $ty),* });
        }
    };
}

macro_rules! stage_error_display {
    ($error:ty, $stage:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}: {self:?}", $stage)
            }
        }

        impl std::error::Error for $error {}
    };
}

pub(crate) use optimization_core::digest_encoder;
#[macro_use]
extern crate selected_instruction_analyses;

use selected_instruction_analyses as analyses;
#[allow(unused_imports)]
pub(crate) use selected_instruction_analyses::*;
use selected_instructions::{FixedViewCopyIdentity, PressureRematerializationIdentity};

#[macro_use]
mod assignment;
mod fixed_view_copy_recovery;
mod home_assignment;
mod output;
mod preservation;
mod pressure_rematerialization_recovery;
mod reanalysis;
mod register_allocation;
mod rewrites;
mod runtime_spill_and_split;
mod selected_input {
    use crate::{
        ValidatedFixedOccurrences, ValidatedFixedViewCopies, ValidatedPressureRematerialization,
        ValidatedRuntimeRematerialization, ValidatedRuntimeSpill, ValidatedRuntimeSplit,
    };
    use optimization_core::OptimizationUnitIdentity;
    use selected_instruction_analyses::{
        OwnedSelectedProgram, SelectedProgramRef, SelectedProgramView,
    };
    use selected_instructions::{
        SelectedInstructionPlan, SelectedInstructionPlanIdentity, SelectedProgram,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    mod sealed {
        pub trait Sealed {}
    }

    pub trait ValidatedSelectedAnalysis: sealed::Sealed + SelectedProgramView {}

    impl sealed::Sealed for SelectedProgram {}
    impl ValidatedSelectedAnalysis for SelectedProgram {}
    impl sealed::Sealed for OwnedSelectedProgram {}
    impl ValidatedSelectedAnalysis for OwnedSelectedProgram {}
    impl sealed::Sealed for SelectedProgramRef<'_> {}
    impl ValidatedSelectedAnalysis for SelectedProgramRef<'_> {}

    macro_rules! validated_selected_analysis {
        ($name:ident) => {
            impl sealed::Sealed for $name {}

            impl ValidatedSelectedAnalysis for $name {}

            impl SelectedProgramView for $name {
                fn selected_plan(&self) -> &SelectedInstructionPlan {
                    self.transformed()
                }
                fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
                    self.shared_transformed()
                }
                fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
                    self.receipt().transformed_selected()
                }
                fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
                    self.receipt().optimization_unit()
                }
                fn work_schedule_identity(&self) -> WorkScheduleIdentity {
                    self.receipt().work_schedule()
                }
            }
        };
    }

    validated_selected_analysis!(ValidatedRuntimeSpill);

    validated_selected_analysis!(ValidatedRuntimeSplit);

    validated_selected_analysis!(ValidatedRuntimeRematerialization);

    validated_selected_analysis!(ValidatedPressureRematerialization);

    validated_selected_analysis!(ValidatedFixedOccurrences);

    impl sealed::Sealed for ValidatedFixedViewCopies {}

    impl ValidatedSelectedAnalysis for ValidatedFixedViewCopies {}

    impl SelectedProgramView for ValidatedFixedViewCopies {
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            std::sync::Arc::clone(&self.plan().transformed)
        }

        fn selected_plan(&self) -> &SelectedInstructionPlan {
            &self.plan().transformed
        }

        fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
            self.receipt().transformed_selected()
        }

        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
            self.receipt().optimization_unit()
        }

        fn work_schedule_identity(&self) -> WorkScheduleIdentity {
            self.receipt().work_schedule()
        }
    }
}

pub use reanalysis::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, stage_optimized_selected_reanalysis,
};
pub(crate) use rewrites::*;
pub use rewrites::{
    FixedOccurrencesError, FixedViewCopy, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    OptimizedFixedViewCopyCustodyError, PressureRematerializationError,
    PressureRematerializationPolicy, RuntimeRematerializationError, RuntimeSpillError,
    RuntimeSpillSpanPolicy, RuntimeSplitError, ValidatedFixedOccurrences, ValidatedFixedViewCopies,
    ValidatedPressureRematerialization, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    ValidatedRuntimeSplit, fixed_view_copy_identity, materialize_selected_fixed_occurrences,
    probe_optimized_fixed_precolored_segment_homes, rematerialize_selected_active_resident,
    rematerialize_selected_runtime_value, spill_selected_runtime_value,
    spill_selected_runtime_value_with_crossing_status,
    spill_selected_runtime_value_with_span_policy, split_selected_runtime_value,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
};
pub use selected_input::ValidatedSelectedAnalysis;

pub(crate) type SelectedLoweringResult =
    selected_instruction_analyses::CompletedSelectedOptimization<
        optimization_core::SelectedLoweringOptimizationCompletionIdentity,
    >;
pub(crate) type PreAllocationResult = selected_instruction_analyses::CompletedSelectedOptimization<
    optimization_core::PreAllocationOptimizationCompletionIdentity,
>;
pub type SelectedLoweringLedger = selected_instruction_analyses::SelectedOptimizationLedger<
    optimization_core::SelectedLoweringOptimizationCompletionIdentity,
>;
pub type PreAllocationLedger = selected_instruction_analyses::SelectedOptimizationLedger<
    optimization_core::PreAllocationOptimizationCompletionIdentity,
>;

pub use register_allocation::{RegisterAllocationError, stage_register_allocation};

pub(crate) use assignment::baseline::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};
pub use assignment::home_assignment::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeIdentity, RegisterHomePlan,
    ValidatedRegisterHomes, VirtualRegisterHome, register_home_identity,
};
pub(crate) use assignment::home_assignment::{
    RegisterHomeError, RegisterHomeValidationReceipt, assign_register_homes,
};
pub use assignment::post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData, ValidatedPostAllocationOptimizationManifest,
};

pub(crate) use assignment::runtime_spill::RuntimeSpillAllocationError;

pub(crate) use assignment::transformed::{
    OptimizedPostPreAllocationHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub(crate) use rewrites::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    stage_optimized_active_resident_rematerialization_pressure,
};

pub(crate) use output::AllocationAdmissionError;
pub use output::{AllocationEvidence, AllocationOutput, RetainedAllocation};

pub use preservation::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedUnitRequirement, CalleeSavedModificationWitness,
    FunctionAllocatedCalleeSavedRequirements, ValidatedAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity, stage_allocated_callee_saved_requirements,
    validate_allocated_callee_saved_requirements,
};

pub use selected_instruction_analyses::{
    MachineEffectStageError, SelectedProgramView, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedPreAllocationMachineEffects, analyze_machine_effects,
};
