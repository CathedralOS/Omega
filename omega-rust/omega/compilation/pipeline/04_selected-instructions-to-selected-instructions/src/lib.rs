#![forbid(unsafe_code)]

/*
We optimize selected machine instructions between target instruction selection
and physical register home assignment (`Omega 04`). Our entry point,
`optimize_selected_instructions` in `selected_optimization.rs`, stages liveness
and live-range facts over the incoming selected control-flow graph (`CFG`) and
either publishes an identity route when no executed optimization rule is
selected or stages allocation legality and runs the selected catalog slice from
`rewrites.rs` (`run_selected_lowering_optimizations` or
`run_pre_allocation_optimizations`), producing one validated
`SelectedInstructionOptimizationOutput`.
*/

/*
Running selected-CFG rewrites before register assignment simplifies operand
chains, eliminates redundant copies and extensions, folds displacements and
constants, and forwards stored loads while virtual registers and explicit flag
publications are still intact. We seal `ValidatedSelectedAnalysis` to
instruction selection's output and our own validated rewrite outputs so an
unchecked `SelectedInstructionPlan` can never enter the shared selected-program
analyses. Every rewrite reconstructs and validates the analyses it invalidates
before handing custody to the next step.
*/

/*
- Input: `SelectedOptimizationInput` from `Omega 03` (`target-operations-to-selected-instructions`).
- Output: `SelectedInstructionOptimizationOutput` consumed by `Omega 05` (`selected-instructions-to-register-homes`).
- Flat module layout:
  - `selected_optimization.rs`: stage admission, catalog phase dispatch, and `SelectedInstructionOptimizationOutput`.
  - `rewrites.rs`: stage rule catalog, shared condition-state/place-storage/window-hazard helpers, and the pre-allocation fixed-point driver.
  - `address_and_load_rewrites.rs`: displacement address folding and byte-exact stored-load forwarding.
  - `copy_and_extension_rewrites.rs`: direct-call copied operand folding, same-block copy removal, and redundant carrier-extension removal.
  - `branch_and_compare_rewrites.rs`: compile-time constant boolean materialization, constant branch folding, and dead/redundant/equivalent compare elimination.
  - `literal_fold_rewrites.rs`: selected-lowering pair-rule catalog and incoming literal folding.
  - `unexecuted_rewrites.rs`: design-record disposition roster for unexecuted rewrite families.
*/

#[macro_use]
extern crate selected_instruction_analyses;

use selected_instruction_analyses as analyses;
use selected_instruction_analyses::work_accounting;

use selected_instruction_analyses::codec_bytes;
use selected_instruction_analyses::cursor;
mod selected_input {
    use crate::{
        ValidatedAddressFold, ValidatedConstantBoolean, ValidatedConstantBranch,
        ValidatedCopiedCallOperand, ValidatedCopyRemoval, ValidatedDeadCompare,
        ValidatedEquivalentCompare, ValidatedLiteralFold, ValidatedPreAllocationTransformation,
        ValidatedRedundantCompare, ValidatedRedundantExtension, ValidatedStoredLoadForwarding,
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

    validated_selected_analysis!(ValidatedAddressFold);

    validated_selected_analysis!(ValidatedConstantBoolean);

    validated_selected_analysis!(ValidatedConstantBranch);

    validated_selected_analysis!(ValidatedCopyRemoval);

    validated_selected_analysis!(ValidatedRedundantCompare);

    validated_selected_analysis!(ValidatedRedundantExtension);

    impl sealed::Sealed for ValidatedPreAllocationTransformation {}

    impl ValidatedSelectedAnalysis for ValidatedPreAllocationTransformation {}

    impl SelectedProgramView for ValidatedPreAllocationTransformation {
        fn selected_plan(&self) -> &SelectedInstructionPlan {
            self.transformed()
        }
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            self.shared_transformed()
        }
        fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
            self.transformed_selected()
        }
        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
            self.optimization_unit()
        }
        fn work_schedule_identity(&self) -> WorkScheduleIdentity {
            self.work_schedule()
        }
    }

    validated_selected_analysis!(ValidatedStoredLoadForwarding);

    validated_selected_analysis!(ValidatedDeadCompare);

    validated_selected_analysis!(ValidatedEquivalentCompare);

    validated_selected_analysis!(ValidatedLiteralFold);

    validated_selected_analysis!(ValidatedCopiedCallOperand);
}
pub use selected_input::ValidatedSelectedAnalysis;
pub use selected_instruction_analyses::SelectedProgramView;
#[macro_use]
mod rewrites;
mod address_and_load_rewrites;
mod branch_and_compare_rewrites;
mod copy_and_extension_rewrites;
mod literal_fold_rewrites;
mod selected_optimization;
pub mod unexecuted_rewrites;

pub use selected_optimization::optimization_output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};

pub use selected_instruction_analyses::SelectedOptimizationInput;
pub use selected_optimization::optimize_selected_instructions;

pub use selected_instruction_analyses::WorkDefect;

pub use analyses::{
    AllocationLegalityError, LiveRangeError, LivenessError, MachineEffectStageError,
    OptimizedAllocationLegalityCustodyError, OwnedSelectedProgram, RecoveryClassificationError,
    RecoveryReanalysisError, SelectedProgramRef, SpillChoiceError,
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    StagedOptimizedLiveRanges, UnsealedRecoveryFacts, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPreAllocationMachineEffects, ValidatedRecoveryClassifications, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_allocation_legality_reusing, analyze_live_ranges,
    analyze_live_ranges_reusing, analyze_liveness, analyze_liveness_reusing,
    analyze_machine_effects, choose_spill_victims, classify_pressure_recovery,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
};
pub(crate) use analyses::{
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError, stage_optimized_live_ranges,
    stage_optimized_liveness,
};

pub use rewrites::unexecuted;
pub(crate) use rewrites::{
    AddressFoldError, ConstantBooleanError, ConstantBranchError, CopiedCallOperandError,
    CopyRemovalError, DeadCompareError, EquivalentCompareError, LiteralFoldPolicy,
    PairResultDisposition, PairUnitDefRelation, PreAllocationPolicy,
    PreAllocationTransformationIdentity, RedundantCompareError, RedundantExtensionError,
    SELECTED_STAGE_RULE_CATALOG, SelectedInstructionPairRule, SelectedLoweringRuleCatalogError,
    SelectedStageRuleRows, resolve_pre_allocation_rules, resolve_selected_lowering_rules,
    run_pre_allocation_optimizations, run_selected_lowering_optimizations,
    validate_pre_allocation_optimization_custody, validate_selected_lowering_optimization_custody,
};
pub(crate) use rewrites::{
    FunctionLiteralFold, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan,
    LiteralFoldValidationReceipt, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedAddressFold, ValidatedConstantBoolean,
    ValidatedConstantBranch, ValidatedCopiedCallOperand, ValidatedCopyRemoval,
    ValidatedDeadCompare, ValidatedEquivalentCompare, ValidatedPreAllocationTransformation,
    ValidatedRedundantCompare, ValidatedRedundantExtension,
};
pub use rewrites::{
    LiteralFoldIdentity, OptimizedLiteralFoldCustodyError, OptimizedPreAllocationCustodyError,
    StagedPreAllocationOptimizationCustodyReceipt, StagedPreAllocationOptimizationRun,
    StagedSelectedLoweringOptimizationCustodyReceipt, StagedSelectedLoweringOptimizationRun,
    ValidatedLiteralFold,
};

pub use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, PressureRematerializationIdentity, RedundantCompareIdentity,
};

pub(crate) use rewrites::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};
