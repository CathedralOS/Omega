#![forbid(unsafe_code)]

/*
We assign physical register homes and execute admitted register-pressure
recovery over the selected instruction program. `stage_register_allocation`
in `register_allocation.rs` consumes the `SelectedAllocationInput` from
selected instruction optimization. It carries the current program, its
validated analyses and its `SelectedOptimizationRecord`. The entry point
runs the allocation procedure the selected allocation-recovery catalog entry
names (or plain assignment), falls back to runtime spilling under remaining
pressure, and publishes one validated `RetainedAllocation` carrying the
current allocated program and its checked facts. The manifest records the
preceding optimization phase for validation.
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
- Native stage 06 (`selected-instructions-to-selected-instructions`) supplies `SelectedAllocationInput`.
- Native stage 08 (`register-homes-to-post-allocation-machine`) consumes `RetainedAllocation` (`output.rs`) and `ValidatedAllocatedCalleeSavedRequirements` (`preservation.rs`).
- Flat module layout:
  - `register_allocation.rs`: the stage entrance: select the recovery catalog entry, take the legality analysis, run the procedure the entry names, admit the result.
  - `assignment.rs`: baseline homes, the three allocation procedures, runtime-spill recovery, and post-allocation manifest staging.
  - `home_assignment.rs`: transition-free constraint solver and interference/early-clobber preparation (no home replay).
  - `rewrites.rs`: fixed-occurrence materialization, fixed-view segment-home staging, settlement-gap alignment, and active-resident rematerialization staging.
  - `fixed_view_copy_recovery.rs`: fixed-view copy materialization (no CFG replay).
  - `pressure_rematerialization_recovery.rs`: active-resident immediate rematerialization under register pressure.
  - `runtime_spill_and_split.rs`: runtime value spilling, live-range splitting, and runtime rematerialization.
  - `reanalysis.rs`: liveness, live-range, and legality reanalysis over fixed-view copy graphs.
  - `output.rs`: `StagedAllocation`, `AllocationOutput` projection and `RetainedAllocation` retention.
  - `publication_validation.rs`: explicit replay of the retained recovery decision and manifest history.
  - `preservation.rs`: allocation-visible callee-saved register modification discovery and replay.
*/

use selected_instruction_analyses as analyses;
pub(crate) use selected_instruction_analyses::*;
use selected_instructions::FixedViewCopyIdentity;

#[macro_use]
mod assignment;
mod fixed_view_copy_recovery;
mod home_assignment;
mod output;
mod preservation;
mod publication_validation;
pub use publication_validation::AllocationPublicationValidationError;
mod pressure_rematerialization_recovery;
mod reanalysis;
mod register_allocation;
mod rewrites;
mod runtime_spill_and_split;
mod selected_input {
    use crate::allocation::{
        MaterializedFixedViewCopies, ValidatedFixedOccurrences, ValidatedPressureRematerialization,
        ValidatedRuntimeRematerialization, ValidatedRuntimeSpill, ValidatedRuntimeSplit,
    };
    use optimization_core::OptimizationUnitIdentity;
    use selected_instruction_analyses::{SelectedProgramView, validated_selected_analysis};
    use selected_instructions::SelectedInstructionPlan;
    use semantic_vocabulary::WorkScheduleIdentity;

    pub use selected_instruction_analyses::ValidatedSelectedAnalysis;

    validated_selected_analysis!(ValidatedRuntimeSpill);
    validated_selected_analysis!(ValidatedRuntimeSplit);
    validated_selected_analysis!(ValidatedRuntimeRematerialization);
    validated_selected_analysis!(ValidatedPressureRematerialization);
    validated_selected_analysis!(ValidatedFixedOccurrences);
    validated_selected_analysis!(@sealed MaterializedFixedViewCopies);

    impl SelectedProgramView for MaterializedFixedViewCopies {
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            std::sync::Arc::clone(&self.plan().transformed)
        }

        fn selected_plan(&self) -> &SelectedInstructionPlan {
            &self.plan().transformed
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
    MaterializedFixedViewCopies, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    OptimizedFixedViewCopyCustodyError, PressureRematerializationError,
    PressureRematerializationPolicy, RuntimeRematerializationError, RuntimeSpillError,
    RuntimeSpillSpanPolicy, RuntimeSplitError, ValidatedFixedOccurrences,
    ValidatedPressureRematerialization, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    ValidatedRuntimeSplit, fixed_view_copy_identity, materialize_selected_fixed_occurrences,
    probe_optimized_fixed_precolored_segment_homes, rematerialize_selected_active_resident,
    rematerialize_selected_runtime_value, spill_selected_runtime_value,
    spill_selected_runtime_value_with_crossing_status, split_selected_runtime_value,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
};
pub use selected_input::ValidatedSelectedAnalysis;

pub use register_allocation::{RegisterAllocationError, stage_register_allocation};

pub(crate) use assignment::baseline::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};
pub use assignment::home_assignment::{
    AssignedRegisterHomes, FunctionRegisterHomes, RegisterHomeIdentity, RegisterHomePlan,
    VirtualRegisterHome, register_home_identity,
};
pub(crate) use assignment::home_assignment::{
    RegisterHomeError, RegisterHomeReceipt, assign_register_homes,
};
pub use assignment::post_allocation_manifest::{
    PostAllocationSelectedTransformation, ValidatedPostAllocationOptimizationManifest,
};

pub(crate) use assignment::runtime_spill::RuntimeSpillAllocationError;

pub(crate) use rewrites::{
    ActiveResidentRematerializationStage, OptimizedActiveResidentRematerializationError,
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    stage_optimized_active_resident_rematerialization_pressure,
};

pub(crate) use output::AllocationAdmissionError;
pub use output::{AllocationOutput, RetainedAllocation};

pub use preservation::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPolicy,
    ValidatedAllocatedCalleeSavedRequirements, stage_allocated_callee_saved_requirements,
};

pub use selected_instruction_analyses::{ValidatedAllocationLegality, ValidatedLiveRanges};
