#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program rewrites before register assignment.
//!
//! Rewrites operate on the selected CFG and reconstruct the analyses they
//! invalidate. Register assignment consumes the resulting current program.
//!
//! Start at [`optimize_selected_instructions`] in `selected_optimization.rs`.
//! It stages [`analyses`] first ([`stage_optimized_liveness`], then
//! [`stage_optimized_live_ranges`]) and publishes the identity route when no
//! executed rule is selected. Otherwise it stages allocation legality and runs
//! exactly one executed catalog slice from [`rewrites`]:
//! [`run_selected_lowering_optimizations`] or
//! [`run_pre_allocation_optimizations`]. Either route publishes one
//! [`SelectedInstructionOptimizationOutput`] from
//! `selected_optimization::optimization_output`.

/// The six analysis identities every validation receipt and analysis input
/// in this stage carries: live ranges, allocation legality, the register
/// environment, allocator availability, the optimization unit and the fuel
/// schedule. Expanded inside each receipt's `impl` so `self.<field>`
/// resolves on the receiver's own field names.
macro_rules! shared_analysis_identity_accessors {
    ($visibility:vis) => {
        $visibility const fn ranges(self) -> LiveRangeIdentity {
            self.ranges
        }
        $visibility const fn legality(self) -> AllocationLegalityIdentity {
            self.legality
        }
        $visibility const fn register_environment(
            self,
        ) -> TargetRegisterEnvironmentIdentity {
            self.register_environment
        }
        $visibility const fn allocator_availability(
            self,
        ) -> AllocatorAvailabilityIdentity {
            self.allocator_availability
        }
        $visibility const fn optimization_unit(self) -> OptimizationUnitIdentity {
            self.optimization_unit
        }
        $visibility const fn fuel_schedule(self) -> FuelScheduleIdentity {
            self.fuel_schedule
        }
    };
}

/// One owner row per receipt accessor: the field's own name read back as its
/// own type. A receipt whose rows are not one of the shared identity blocks
/// lists them here instead of restating the body at the site; `ref` selects
/// the `&self` receiver for receipts whose callers hold a reference.
macro_rules! receipt_accessor_rows {
    (ref $name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { self.$field } )*
    };
    ($name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(self) -> $ty { self.$field } )*
    };
}

/// The producer rows a staged shell re-serves unchanged: one internal rule per
/// accessor, so a shell names its producer field and the rows it re-serves
/// instead of restating each delegation body and its doc. `$producer` is the
/// field holding the retained producer stage, and every row delegates to that
/// producer's own method of the same name.
macro_rules! staged_producer_accessors {
    ($producer:ident; $($row:ident),* $(,)?) => {
        $( staged_producer_accessors!(@row $producer, $row); )*
    };
    (@row $producer:ident, selected) => {
        /// The current selected program this stage describes.
        pub const fn selected(&self) -> &ValidatedSelectedInstructions {
            self.$producer.selected()
        }
    };
    (@row $producer:ident, liveness) => {
        /// The liveness facts over the current program.
        pub const fn liveness(&self) -> &ValidatedLiveness {
            self.$producer.liveness()
        }
    };
    (@row $producer:ident, ranges) => {
        /// The live ranges over the current program.
        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            self.$producer.ranges()
        }
    };
    (@row $producer:ident, register_environment) => {
        /// The target register environment admitted with the current program.
        pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
            self.$producer.register_environment()
        }
    };
    (@row $producer:ident, allocator_availability) => {
        /// The environment-derived allocator availability governing the program.
        pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
            self.$producer.allocator_availability()
        }
    };
    (@row $producer:ident, source_legality_stage) => {
        /// The source legality stage this stage's program was derived from.
        pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
            self.$producer.source_legality_stage()
        }
    };
    (@row $producer:ident, selections) => {
        /// The governing optimizer selections for this admission.
        pub fn selections(&self) -> &OptimizationSelections {
            self.$producer.selections()
        }
    };
    (@row $producer:ident, budget_per_pass) => {
        /// The per-pass work budget admitted beside the same evidence.
        pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
            self.$producer.budget_per_pass()
        }
    };
}

/// The validated analyses a staged optimization keeps: liveness, live
/// ranges, and allocation legality.
macro_rules! validated_analysis_accessors {
    () => {
        pub const fn liveness(&self) -> &ValidatedLiveness {
            &self.liveness
        }
        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            &self.ranges
        }
        pub const fn legality(&self) -> &ValidatedAllocationLegality {
            &self.legality
        }
    };
}

/// `From<WorkDefect>` for every bounded stage error that carries
/// `Work(WorkDefect)`: a charge's `?` lands in the stage's own refusal surface
/// without a per-stage adapter block or a per-site mapping.
macro_rules! work_defect_conversion {
    ($($error:ty),+ $(,)?) => {$(
        impl From<WorkDefect> for $error {
            fn from(defect: WorkDefect) -> Self {
                Self::Work(defect)
            }
        }
    )+};
}

/// `Display` for the crate's `<context> failed: {self:?}` error convention.
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

mod analyses;
mod codec_bytes;
mod cursor;
mod rewrites;
mod selected_optimization;
mod work_accounting;

// The stage entry and the output it publishes.
pub use selected_optimization::optimization_output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};

pub use selected_optimization::optimize_selected_instructions;

// The refusal every bounded pass in this stage shares.
pub use work_accounting::WorkDefect;

// Current selected-program facts and their independent validation.
pub(crate) use analyses::materialize_allocator_availability;
pub use analyses::{
    AllocationLegalityError, LiveRangeError, LivenessError, MachineEffectStageError,
    OptimizedAllocationLegalityCustodyError, OptimizedSelectedReanalysisError,
    OwnedSelectedProgram, RecoveryClassificationError, RecoveryReanalysisError, SelectedProgramRef,
    SpillChoiceError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiveRanges,
    StagedOptimizedSelectedReanalysis, StagedOptimizedSelectedReanalysisCustodyReceipt,
    UnsealedRecoveryFacts, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
    ValidatedLiveRanges, ValidatedLiveness, ValidatedPreAllocationMachineEffects,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    analyze_allocation_legality, analyze_allocation_legality_reusing, analyze_live_ranges,
    analyze_live_ranges_reusing, analyze_liveness, analyze_liveness_reusing,
    analyze_machine_effects, choose_spill_victims, classify_pressure_recovery,
    stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_selected_reanalysis,
};
pub(crate) use analyses::{
    AllocationLegalityValidationReceipt, AllocatorAvailabilityError,
    AllocatorAvailabilityValidationReceipt, FixedPrecoloredIntervalValidationReceipt,
    FixedPrecoloredSegmentHomeValidationReceipt, FixedPrecoloredSplitRequirementValidationReceipt,
    LiveRangeValidationReceipt, LivenessValidationReceipt, RecoveryClassificationValidationReceipt,
    SpillChoiceValidationReceipt, StagedOptimizedLiveRangeCustodyReceipt,
    StagedOptimizedLivenessCustodyReceipt,
};
pub(crate) use analyses::{
    FixedPrecoloredIntervalError, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError, MachineEffectError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, ValidatedFixedPrecoloredIntervals,
    ValidatedFixedPrecoloredSegmentHomes, ValidatedFixedPrecoloredSplitRequirements,
    analyze_fixed_precolored_intervals, analyze_fixed_precolored_split_requirements,
    assign_fixed_precolored_segment_homes, stage_optimized_live_ranges, stage_optimized_liveness,
};

// The five bounded stages of this group carry the shared work refusal.
work_defect_conversion!(
    FixedPrecoloredIntervalError,
    FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError,
    RecoveryClassificationError,
    SpillChoiceError,
);
// Selected-CFG rewrites: the executed catalog slices, the recovery rewrites
// register assignment replays, and the unexecuted families.
pub use rewrites::unexecuted;
pub(crate) use rewrites::{
    AddressFoldError, ConstantBooleanError, ConstantBranchError, CopiedCallOperandError,
    CopyRemovalError, DeadCompareError, EquivalentCompareError, FixedViewCopyError,
    LiteralFoldPolicy, PairOperandShape, PairResultDisposition, PairUnitDefRelation,
    PreAllocationPolicy, PreAllocationTransformationIdentity, RedundantCompareError,
    RedundantExtensionError, SELECTED_STAGE_RULE_CATALOG, SelectedInstructionPairRule,
    SelectedLoweringRuleCatalogError, SelectedStageRuleRows,
    StagedOptimizedFixedPrecoloredSegmentHomes, StagedOptimizedFixedViewCopies,
    resolve_pre_allocation_rules, resolve_selected_lowering_rules,
    run_pre_allocation_optimizations, run_selected_lowering_optimizations,
    validate_pre_allocation_optimization_custody, validate_selected_lowering_optimization_custody,
};
pub use rewrites::{
    AllocationRecoveryRuleCatalogError, FixedViewCopy, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, LiteralFoldIdentity,
    OptimizedFixedPrecoloredSegmentHomeCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedPreAllocationCustodyError,
    PressureRematerializationError, PressureRematerializationPolicy, RuntimeRematerializationError,
    RuntimeSpillError, RuntimeSpillSpanPolicy, StagedOptimizedLiteralFoldCustodyReceipt,
    StagedOptimizedLiteralFolds, StagedPreAllocationOptimizationCustodyReceipt,
    StagedPreAllocationOptimizationRun, StagedSelectedLoweringOptimizationCustodyReceipt,
    StagedSelectedLoweringOptimizationRun, ValidatedFixedViewCopies, ValidatedLiteralFold,
    ValidatedPressureRematerialization, ValidatedRuntimeRematerialization, ValidatedRuntimeSpill,
    probe_optimized_fixed_precolored_segment_homes, rematerialize_selected_active_resident,
    rematerialize_selected_runtime_value, selected_allocation_recovery_rule,
    spill_selected_runtime_value, spill_selected_runtime_value_with_crossing_status,
    spill_selected_runtime_value_with_span_policy, stage_optimized_fixed_precolored_segment_homes,
    stage_optimized_fixed_view_copies,
};
pub(crate) use rewrites::{
    FixedViewCopyDestination, FixedViewCopyValidationReceipt, FunctionLiteralFold,
    FunctionPressureRematerialization, LiteralFoldAction, LiteralFoldError, LiteralFoldPlan,
    LiteralFoldValidationReceipt, PressureRematerializationAction, PressureRematerializationPlan,
    PressureRematerializationRewrite, PressureRematerializationValidationReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedViewCopyCustodyReceipt, StagedOptimizedLiteralFoldAttempt,
    StagedOptimizedLiteralFoldAttemptReceipt, StagedOptimizedLiteralFoldIterationReceipt,
    StagedOptimizedLiteralFoldStep, StagedOptimizedPreAllocationAttempt,
    StagedOptimizedPreAllocationAttemptReceipt, StagedOptimizedPreAllocationIterationReceipt,
    StagedOptimizedPreAllocationStep, ValidatedAddressFold, ValidatedConstantBoolean,
    ValidatedConstantBranch, ValidatedCopiedCallOperand, ValidatedCopyRemoval,
    ValidatedDeadCompare, ValidatedEquivalentCompare, ValidatedPreAllocationTransformation,
    ValidatedRedundantCompare, ValidatedRedundantExtension, fixed_view_copy_identity,
};
pub use rewrites::{RuntimeSplitError, ValidatedRuntimeSplit, split_selected_runtime_value};

// Durable transform-output identities live with the selected-instruction
// representation; the transform publishes them under its root so callers keep
// one vocabulary.
pub use selected_instructions::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, PressureRematerializationIdentity, RedundantCompareIdentity,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

pub(crate) use rewrites::{
    StoredLoadForwardingError, ValidatedStoredLoadForwarding, forward_selected_stored_load,
};

pub use rewrites::{
    FixedOccurrencesError, ValidatedFixedOccurrences, materialize_selected_fixed_occurrences,
};
