#![forbid(unsafe_code)]

/*
  Pre-allocation dataflow, live-range, machine-effect, and register-allocation
  legality analyses (`ValidatedLiveness`, `ValidatedLiveRanges`,
  `ValidatedAllocationLegality`, `ValidatedPreAllocationMachineEffects`) and
  staged custody carriers for `SelectedInstructionPlan`.

  We compute and seal the target-aware program facts that govern pre-allocation
  rewrites and physical register assignment: backward CFG liveness (`liveness`),
  live-range fragments and interference graphs (`live_ranges`), per-instruction
  machine effects (`machine_effects`), allocator register availability
  (`allocator_availability`), candidate register-view legality (`allocation_legality`,
  `legality`), fixed/precolored intervals, segment homes, and split cuts
  (`fixed_precolored_intervals`, `fixed_precolored_segment_homes`,
  `fixed_precolored_split_requirements`), and deterministic spill-victim selection
  and recovery classification (`spill_choice`, `recovery_classification`).
  Shared rewrite primitives (`rewriting`) and bounded work-budget accounting
  (`work_accounting`) support both optimization passes and pressure recovery.
*/

/*
  Selected optimization (`06_selected-instructions-to-selected-instructions`)
  and register allocation (`07_selected-instructions-to-register-homes`) both
  require authenticated liveness, live-range, and legality facts over the
  same `SelectedInstructionPlan` representation. Housing these analyses in a
  single representation-analysis crate lets both pipeline stages share the
  exact dataflow and interference engines, while `UnsealedRecoveryFacts` and
  `carried_digests` reuse unchanged per-function `Arc` rows without
  re-analyzing or re-hashing untouched functions: across stage 07's iterative
  spill/split recovery steps, and through `reanalyze_rewritten_functions`
  after each rewrite stage 06 commits.
*/

/*
  Consumes `SelectedProgram`, `SelectedInstructionPlan`, and target register
  models (`ValidatedTargetRegisterEnvironment`, `ValidatedPhysicalRegisterModel`)
  through `SelectedProgramView`. Produces sealed analysis carriers and custody
  receipts (`StagedOptimizedLiveness`, `StagedOptimizedLiveRanges`,
  `StagedOptimizedAllocationLegality`) consumed by
  `06_selected-instructions-to-selected-instructions` and
  `07_selected-instructions-to-register-homes`. `SelectedAllocationInput` is the
  one form Omega 04 hands Omega 05: the current program, the analyses Omega 04
  already established for it, and its `SelectedOptimizationRecord`. Omega 04's
  rewrite history is evidence in that record, never a choice of how Omega 05
  allocates.
*/

#[macro_export]
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
        $visibility const fn work_schedule(self) -> WorkScheduleIdentity {
            self.work_schedule
        }
    };
}

#[macro_export]
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
    (nested $name:ident, $row:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(self) -> $ty { self.$row.$field() } )*
    };
}

#[macro_export]
macro_rules! copy_receipt {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $fvis:vis $field:ident : $ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(
                $(#[$fmeta])*
                $fvis $field: $ty
            ),*
        }

        impl $name {
            receipt_accessor_rows!($name {
                $($(#[$fmeta])* $field: $ty),*
            });
        }
    };
}

#[macro_export]
macro_rules! staged_producer_accessors {
    ($producer:ident; $($row:ident),* $(,)?) => {
        $( staged_producer_accessors!(@row $producer, $row); )*
    };
    (@row $producer:ident, selected) => {
        pub const fn selected(&self) -> &SelectedProgram {
            self.$producer.selected()
        }
    };
    (@row $producer:ident, liveness) => {
        pub const fn liveness(&self) -> &ValidatedLiveness {
            self.$producer.liveness()
        }
    };
    (@row $producer:ident, ranges) => {
        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            self.$producer.ranges()
        }
    };
    (@row $producer:ident, register_environment) => {
        pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
            self.$producer.register_environment()
        }
    };
    (@row $producer:ident, allocator_availability) => {
        pub const fn allocator_availability(&self) -> &ValidatedAllocatorAvailability {
            self.$producer.allocator_availability()
        }
    };
    (@row $producer:ident, source_legality_stage) => {
        pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
            self.$producer.source_legality_stage()
        }
    };
    (@row $producer:ident, record) => {
        pub const fn record(&self) -> &$crate::SelectedOptimizationRecord {
            self.$producer.record()
        }
    };
    (@row $producer:ident, selections) => {
        pub fn selections(&self) -> &OptimizationSelections {
            self.$producer.selections()
        }
    };
    (@row $producer:ident, budget_per_pass) => {
        pub fn budget_per_pass(&self) -> OptimizationWorkBudget {
            self.$producer.budget_per_pass()
        }
    };
}

#[macro_export]
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

#[macro_export]
macro_rules! work_defect_conversion {
    ($($error:ty),+ $(,)?) => {$(
        impl From<WorkDefect> for $error {
            fn from(defect: WorkDefect) -> Self {
                Self::Work(defect)
            }
        }
    )+};
}

#[macro_export]
macro_rules! validated_plan_carrier {
    ($carrier:ident : $plan:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $carrier {
            pub(crate) plan: $plan,
        }

        impl $carrier {
            pub const fn plan(&self) -> &$plan {
                &self.plan
            }
        }
    };
    ($carrier:ident : $plan:ty, $receipt:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $carrier {
            pub(crate) plan: $plan,
            pub(crate) receipt: $receipt,
        }

        impl $carrier {
            pub const fn plan(&self) -> &$plan {
                &self.plan
            }

            receipt_accessor_rows!(ref $carrier {
                receipt: $receipt,
            });
        }
    };
}

#[macro_export]
macro_rules! shared_plan_carrier {
    (
        $carrier:ident : $plan:ty, $receipt:ty {
            $( $(#[$meta:meta])* $vis:vis $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $carrier {
            pub(crate) plan: std::sync::Arc<$plan>,
            pub(crate) receipt: $receipt,
            pub(crate) function_digests: std::sync::Arc<[[u8; 32]]>,
            $($(#[$meta])* $vis $field: $ty,)*
        }

        impl $carrier {
            pub fn plan(&self) -> &$plan {
                &self.plan
            }

            receipt_accessor_rows!(ref $carrier {
                receipt: $receipt,
            });
        }
    };
}

#[macro_export]
macro_rules! seal_plan {
    (
        $seal:ident($plan:ty) -> $carrier:ident : $receipt:ident
            = $identity:path, $error:ty
        { $($field:ident),* $(,)? }
    ) => {
        pub(crate) fn $seal(candidate: $plan) -> Result<$carrier, $error> {
            let receipt = $receipt {
                identity: $identity(&candidate),
                $($field: candidate.$field,)*
            };
            Ok($carrier {
                plan: candidate,
                receipt,
            })
        }
    };
}

#[macro_export]
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }

        impl std::error::Error for $type {}
    };
}

#[macro_export]
macro_rules! validated_transformed_receipt {
    (
        $(#[$carrier_doc:meta])*
        $name:ident, $receipt:ident
    ) => {
        $(#[$carrier_doc])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            transformed: Arc<SelectedInstructionPlan>,
            receipt: $receipt,
        }

        impl $name {
            pub fn transformed(&self) -> &SelectedInstructionPlan {
                &self.transformed
            }

            pub fn shared_transformed(&self) -> Arc<SelectedInstructionPlan> {
                Arc::clone(&self.transformed)
            }

            pub const fn receipt(&self) -> &$receipt {
                &self.receipt
            }
        }
    };
}

#[macro_export]
macro_rules! receipt_selected_identities {
    ($name:ident) => {
        impl $name {
            pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
                self.source_selected
            }
            pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
                self.transformed_selected
            }
            pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
                self.optimization_unit
            }
            pub const fn work_schedule(&self) -> WorkScheduleIdentity {
                self.work_schedule
            }
        }
    };
}

#[macro_export]
macro_rules! rewrite_receipt {
    (
        $receipt:ident {
            $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $receipt {
            source_selected: SelectedInstructionPlanIdentity,
            transformed_selected: SelectedInstructionPlanIdentity,
            optimization_unit: OptimizationUnitIdentity,
            work_schedule: WorkScheduleIdentity,
            $($(#[$meta])* $field: $ty,)*
        }

        impl $receipt {
            receipt_accessor_rows!(ref $receipt {
                $($(#[$meta])* $field: $ty),*
            });
        }

        receipt_selected_identities!($receipt);
    };
    ($receipt:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $receipt {
            source_selected: SelectedInstructionPlanIdentity,
            transformed_selected: SelectedInstructionPlanIdentity,
            optimization_unit: OptimizationUnitIdentity,
            work_schedule: WorkScheduleIdentity,
        }

        receipt_selected_identities!($receipt);
    };
}

#[macro_export]
macro_rules! rewrite_receipt_identity {
    (
        $function:ident, $identity:ident, $receipt:ident,
        $separator:literal, $phrase:literal, $($coordinate:ident),+ $(,)?
    ) => {
        pub(crate) fn $function(receipt: &$receipt) -> $identity {
            $identity::from_canonical_bytes(&$crate::rewriting::rewrite_bytes(
                $separator,
                &receipt.source_selected.bytes(),
                &receipt.transformed_selected.bytes(),
                &receipt.optimization_unit.bytes(),
                |bytes| {
                    $crate::codec_bytes::push_encoded(bytes, receipt.work_schedule.marker().to_le_bytes());
                    bytes.extend_from_slice(
                        &u64::try_from(receipt.function_index)
                            .expect(concat!($phrase, " function index fits u64"))
                            .to_le_bytes(),
                    );
                    $($crate::codec_bytes::push_encoded(bytes, receipt.$coordinate.0.to_le_bytes());)+
                },
            ))
        }

        impl $receipt {
            pub fn identity(&self) -> $identity {
                $function(self)
            }
        }
    };
}

mod allocation_recovery_catalog {

    use optimization_core::{
        Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
        OptimizationPhaseMismatch, OptimizationPhaseSelections,
    };
    use register_homes::{
        FixedViewCopyPolicy, PressureRematerializationPolicy, RecoveryClassificationPolicy,
        SpillChoicePolicy,
    };

    use crate::AllocatorAvailabilityRequirement;
    use crate::rewriting::RegisterAllocationRuleTargetApplicability;

    /// The recovery procedure an allocation-recovery entry runs, with every
    /// policy it uses. Omega 05 runs the procedure the selected entry names;
    /// it keeps no second table from optimization names to procedures.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AllocationRecoveryRule {
        /// Split fixed-register conflicts with explicit copies, then assign.
        FixedViewCopies(FixedViewCopyPolicy),
        /// Rematerialize a chosen immediate before its uses, then assign.
        PressureRematerialization {
            choice: SpillChoicePolicy,
            classification: RecoveryClassificationPolicy,
            rematerialization: PressureRematerializationPolicy,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AllocationRecoveryRuleCatalogPayload {
        target: RegisterAllocationRuleTargetApplicability,
        availability: AllocatorAvailabilityRequirement,
        rule: AllocationRecoveryRule,
    }

    impl AllocationRecoveryRuleCatalogPayload {
        /// The allocator availability the rule's legality analysis must use.
        pub const fn availability(&self) -> AllocatorAvailabilityRequirement {
            self.availability
        }

        pub const fn rule(&self) -> AllocationRecoveryRule {
            self.rule
        }

        pub const fn target(&self) -> RegisterAllocationRuleTargetApplicability {
            self.target
        }
    }

    pub type AllocationRecoveryRuleCatalogEntry =
        OptimizationCatalogDescriptor<AllocationRecoveryRuleCatalogPayload>;

    pub const ALLOCATION_RECOVERY_RULE_CATALOG: [AllocationRecoveryRuleCatalogEntry; 2] = [
        AllocationRecoveryRuleCatalogEntry::new(
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            AllocationRecoveryRuleCatalogPayload {
                target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
                availability: AllocatorAvailabilityRequirement::AllEnvironmentAllocatableViews,
                rule: AllocationRecoveryRule::FixedViewCopies(
                    FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
                ),
            },
        ),
        AllocationRecoveryRuleCatalogEntry::new(
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            AllocationRecoveryRuleCatalogPayload {
                target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
                availability:
                    AllocatorAvailabilityRequirement::ActiveResidentImmediateU64MultiUseRematerializationV1,
                rule: AllocationRecoveryRule::PressureRematerialization {
                    choice: SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
                    classification:
                        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
                    rematerialization: PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
                },
            },
        ),
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AllocationRecoveryRuleCatalogError {
        WrongPhase(OptimizationPhaseMismatch),
        UnsupportedSelection(Optimization),
        UnsupportedComposition,
    }

    /// The catalog entry the allocation-recovery projection selects, or none.
    /// More than one selected rule is an unsupported composition.
    pub fn selected_allocation_recovery_entry(
        selections: &OptimizationPhaseSelections,
    ) -> Result<
        Option<&'static AllocationRecoveryRuleCatalogEntry>,
        AllocationRecoveryRuleCatalogError,
    > {
        let phase = selections
            .require_phase(OptimizationExecutionPhase::AllocationRecovery)
            .map_err(AllocationRecoveryRuleCatalogError::WrongPhase)?;
        match phase.as_slice() {
            [] => Ok(None),
            [selected] => ALLOCATION_RECOVERY_RULE_CATALOG
                .iter()
                .find(|entry| entry.optimization() == *selected)
                .map(Some)
                .ok_or(AllocationRecoveryRuleCatalogError::UnsupportedSelection(
                    *selected,
                )),
            _ => Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition),
        }
    }
}
pub use allocation_recovery_catalog::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRule, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    selected_allocation_recovery_entry,
};
pub mod codec_bytes {

    pub use ::wire::codec_bytes::push_encoded;
    pub use ::wire::codec_bytes::{decode_header, encode_header};
}
mod selected_optimization_record {

    use optimization_core::{
        PreAllocationOptimizationCompletionIdentity, SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_homes::PostAllocationSelectedTransformation;

    /// What earlier selected-instruction rewrites did to the program a stage
    /// receives: the completed Omega 04 phase, if any, and the transformations
    /// it applied in order. Allocation copies it into its manifest as
    /// evidence; it never chooses how a later stage runs. Instruction
    /// selection's output carries the empty record.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct SelectedOptimizationRecord {
        pub selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
        pub pre_allocation_completion: Option<PreAllocationOptimizationCompletionIdentity>,
        pub transformations: Vec<PostAllocationSelectedTransformation>,
    }

    impl SelectedOptimizationRecord {
        /// Whether an Omega 04 rewrite phase produced the program.
        pub const fn is_rewritten(&self) -> bool {
            self.selected_lowering_completion.is_some() || self.pre_allocation_completion.is_some()
        }

        /// This record followed by transformations a later stage applied.
        pub fn followed_by(
            &self,
            later: &[PostAllocationSelectedTransformation],
        ) -> Vec<PostAllocationSelectedTransformation> {
            let mut transformations = Vec::with_capacity(self.transformations.len() + later.len());
            transformations.extend_from_slice(&self.transformations);
            transformations.extend_from_slice(later);
            transformations
        }
    }
}
pub use selected_optimization_record::SelectedOptimizationRecord;
mod allocation_input;
pub use ::wire::cursor;
pub use allocation_input::SelectedAllocationInput;
pub mod rewriting;
pub mod work_accounting;
pub use work_accounting::WorkDefect;
mod optimization_input {

    use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use selected_instructions::{SelectedProgram, StagedOptimizedCustodyCore};

    use crate::SelectedOptimizationRecord;

    /// One selected program with the custody, registers, selections and
    /// budgets a selected-instruction stage works under. `record` says which
    /// earlier selected-instruction rewrites produced `program`; it is empty
    /// for instruction selection's own output.
    #[derive(Debug)]
    pub struct SelectedOptimizationInput {
        pub program: SelectedProgram,
        pub custody: StagedOptimizedCustodyCore,
        pub register_environment: ValidatedTargetRegisterEnvironment,
        pub selections: OptimizationSelections,
        pub budget_per_pass: OptimizationWorkBudget,
        pub record: SelectedOptimizationRecord,
    }

    impl SelectedOptimizationInput {
        pub const fn selected(&self) -> &SelectedProgram {
            &self.program
        }

        pub const fn record(&self) -> &SelectedOptimizationRecord {
            &self.record
        }

        pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
            &self.register_environment
        }

        pub const fn selections(&self) -> &OptimizationSelections {
            &self.selections
        }

        pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
            self.budget_per_pass
        }
    }
}
pub use optimization_input::SelectedOptimizationInput;

mod legality;
pub mod live_ranges;
pub mod liveness;
pub mod machine_effects;
mod tied_registers;

pub use legality::{
    AllocatorAvailabilityRequirement, OptimizedAllocationLegalityCustodyError,
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    admit_optimized_allocation_legality, stage_optimized_allocation_legality,
};
pub use live_ranges::{
    LiveRangeError, StagedOptimizedLiveRanges, ValidatedLiveRanges, analyze_live_ranges,
};
pub use live_ranges::{
    LiveRangeValidationReceipt, OptimizedLiveRangeCustodyError,
    StagedOptimizedLiveRangeCustodyReceipt, admit_optimized_live_ranges,
    stage_optimized_live_ranges,
};
pub use liveness::{LivenessError, ValidatedLiveness, analyze_liveness};
pub use liveness::{
    LivenessValidationReceipt, OptimizedLivenessCustodyError, StagedOptimizedLiveness,
    StagedOptimizedLivenessCustodyReceipt, admit_optimized_liveness, stage_optimized_liveness,
};
pub use machine_effects::MachineEffectError;
pub use machine_effects::analyze_pre_allocation_machine_effects;
pub use machine_effects::prepared_machine_effect_catalog;
pub use machine_effects::{
    MachineEffectStageError, ValidatedPreAllocationMachineEffects, analyze_machine_effects,
};

pub mod allocation_legality;
pub mod allocator_availability;
pub mod fixed_precolored_intervals;
mod fixed_precolored_segment_homes;
pub mod fixed_precolored_split_requirements;
pub mod recovery_classification;
pub mod register_writes;
mod selected_input {
    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{
        CurrentSelectedProgram, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
        SelectedProgram,
    };
    use semantic_vocabulary::WorkScheduleIdentity;

    pub trait SelectedProgramView {
        fn selected_plan(&self) -> &SelectedInstructionPlan;
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan>;
        fn selected_identity(&self) -> SelectedInstructionPlanIdentity;
        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity;
        fn work_schedule_identity(&self) -> WorkScheduleIdentity;

        fn current_program(&self) -> CurrentSelectedProgram<'_> {
            CurrentSelectedProgram {
                plan: self.selected_plan(),
                identity: self.selected_identity(),
                optimization_unit: self.optimization_unit_identity(),
                work_schedule: self.work_schedule_identity(),
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct SelectedProgramRef<'program> {
        program: &'program dyn SelectedProgramView,
    }

    impl<'program> SelectedProgramRef<'program> {
        pub fn plan(self) -> &'program SelectedInstructionPlan {
            self.program.selected_plan()
        }

        pub fn new(program: &'program impl SelectedProgramView) -> Self {
            Self { program }
        }
    }

    impl SelectedProgramView for SelectedProgramRef<'_> {
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            self.program.shared_selected_plan()
        }

        fn selected_plan(&self) -> &SelectedInstructionPlan {
            self.program.selected_plan()
        }

        fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
            self.program.selected_identity()
        }

        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
            self.program.optimization_unit_identity()
        }

        fn work_schedule_identity(&self) -> WorkScheduleIdentity {
            self.program.work_schedule_identity()
        }
    }

    impl SelectedProgramView for SelectedProgram {
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            self.shared_plan()
        }

        fn selected_plan(&self) -> &SelectedInstructionPlan {
            self.plan()
        }

        fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
            self.identity()
        }

        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
            self.optimization_unit()
        }

        fn work_schedule_identity(&self) -> WorkScheduleIdentity {
            self.work_schedule()
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct OwnedSelectedProgram {
        plan: std::sync::Arc<SelectedInstructionPlan>,
        selected: SelectedInstructionPlanIdentity,
        optimization_unit: OptimizationUnitIdentity,
        work_schedule: WorkScheduleIdentity,
    }

    impl OwnedSelectedProgram {
        pub fn retain(source: &impl SelectedProgramView) -> Self {
            Self {
                plan: source.shared_selected_plan(),
                selected: source.selected_identity(),
                optimization_unit: source.optimization_unit_identity(),
                work_schedule: source.work_schedule_identity(),
            }
        }
    }

    impl SelectedProgramView for OwnedSelectedProgram {
        fn selected_plan(&self) -> &SelectedInstructionPlan {
            &self.plan
        }
        fn shared_selected_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
            std::sync::Arc::clone(&self.plan)
        }
        fn selected_identity(&self) -> SelectedInstructionPlanIdentity {
            self.selected
        }
        fn optimization_unit_identity(&self) -> OptimizationUnitIdentity {
            self.optimization_unit
        }
        fn work_schedule_identity(&self) -> WorkScheduleIdentity {
            self.work_schedule
        }
    }

    /// A selected program a stage may treat as already validated: instruction
    /// selection's output, a retained copy or view of it, or a stage's own
    /// validated rewrite result. Stage code that must never receive an
    /// unchecked `SelectedInstructionPlan` bounds on this trait. The trait is
    /// sealed; stages 06 and 07 admit their rewrite results through
    /// `validated_selected_analysis!`.
    pub trait ValidatedSelectedAnalysis:
        crate::validated_selected_support::Sealed + SelectedProgramView
    {
    }

    crate::validated_selected_analysis!(@sealed SelectedProgram);
    crate::validated_selected_analysis!(@sealed OwnedSelectedProgram);
    crate::validated_selected_analysis!(@sealed SelectedProgramRef<'_>);
}

/// Names `validated_selected_analysis!` expands to in the stage crates.
#[doc(hidden)]
pub mod validated_selected_support {
    pub use optimization_core::OptimizationUnitIdentity;
    pub use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
    pub use semantic_vocabulary::WorkScheduleIdentity;

    /// Keeps `ValidatedSelectedAnalysis` closed to the types the macro admits.
    pub trait Sealed {}
}

/// Admits a validated rewrite result as a `ValidatedSelectedAnalysis`.
///
/// `validated_selected_analysis!(Name)` also implements `SelectedProgramView`
/// for a type that exposes `transformed()`, `shared_transformed()` and a
/// `receipt()` naming the transformed program, its optimization unit and its
/// work schedule. `validated_selected_analysis!(@sealed Name)` only admits the
/// type, for one that implements `SelectedProgramView` itself.
#[macro_export]
macro_rules! validated_selected_analysis {
    (@sealed $name:ty) => {
        impl $crate::validated_selected_support::Sealed for $name {}
        impl $crate::ValidatedSelectedAnalysis for $name {}
    };
    ($name:ident) => {
        $crate::validated_selected_analysis!(@sealed $name);

        impl $crate::SelectedProgramView for $name {
            fn selected_plan(&self) -> &$crate::validated_selected_support::SelectedInstructionPlan {
                self.transformed()
            }
            fn shared_selected_plan(
                &self,
            ) -> std::sync::Arc<$crate::validated_selected_support::SelectedInstructionPlan> {
                self.shared_transformed()
            }
            fn selected_identity(
                &self,
            ) -> $crate::validated_selected_support::SelectedInstructionPlanIdentity {
                self.receipt().transformed_selected()
            }
            fn optimization_unit_identity(
                &self,
            ) -> $crate::validated_selected_support::OptimizationUnitIdentity {
                self.receipt().optimization_unit()
            }
            fn work_schedule_identity(
                &self,
            ) -> $crate::validated_selected_support::WorkScheduleIdentity {
                self.receipt().work_schedule()
            }
        }
    };
}
pub mod spill_choice;
pub use allocation_legality::AllocationLegalityValidationReceipt;
pub use allocation_legality::{
    AllocationLegalityError, ValidatedAllocationLegality, analyze_allocation_legality,
};
pub use allocator_availability::AllocatorAvailabilityError;
pub use allocator_availability::ValidatedAllocatorAvailability;
pub use allocator_availability::materialize_allocator_availability;
pub use fixed_precolored_intervals::{
    FixedPrecoloredIntervalError, ValidatedFixedPrecoloredIntervals,
    analyze_fixed_precolored_intervals,
};
pub use fixed_precolored_segment_homes::{
    FixedPrecoloredSegmentHomeError, ValidatedFixedPrecoloredSegmentHomes,
    assign_fixed_precolored_segment_homes,
};
pub use fixed_precolored_split_requirements::{
    FixedPrecoloredSplitRequirementError, ValidatedFixedPrecoloredSplitRequirements,
    analyze_fixed_precolored_split_requirements,
};
pub use recovery_classification::{
    RecoveryClassificationError, ValidatedRecoveryClassifications, classify_pressure_recovery,
};
pub use selected_input::{
    OwnedSelectedProgram, SelectedProgramRef, SelectedProgramView, ValidatedSelectedAnalysis,
};

work_defect_conversion!(
    FixedPrecoloredIntervalError,
    FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError,
    RecoveryClassificationError,
    SpillChoiceError,
);

pub use spill_choice::{SpillChoiceError, ValidatedSpillChoices, choose_spill_victims};
use std::sync::Arc;

pub(crate) fn carried_digests<Function>(
    functions: &[Function],
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    digest: impl Fn(&Function) -> [u8; 32],
) -> Arc<[[u8; 32]]> {
    functions
        .iter()
        .enumerate()
        .map(|(index, function)| match previous_digests.get(index) {
            Some(previous) if carried.get(index) == Some(&true) => *previous,
            _ => digest(function),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryReanalysisError {
    Liveness(LivenessError),
    Ranges(LiveRangeError),
    Legality(AllocationLegalityError),
}

#[derive(Clone)]
pub struct UnsealedRecoveryFacts {
    selected: selected_instructions::SelectedInstructionPlanIdentity,
    availability: register_homes::AllocatorAvailabilityIdentity,
    liveness_functions: Vec<Arc<selected_instructions::FunctionLiveness>>,
    liveness_materialized: Vec<bool>,
    range_functions: Vec<Arc<selected_instructions::FunctionLiveRanges>>,
    legality_functions: Vec<Arc<register_homes::FunctionAllocationLegality>>,
}

impl UnsealedRecoveryFacts {
    pub fn from_validated(
        liveness: &ValidatedLiveness,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
    ) -> Self {
        let count = liveness.plan().functions.len();
        Self {
            selected: liveness.plan().selected,
            availability: legality.plan().allocator_availability,
            liveness_functions: liveness.plan().functions.clone(),
            liveness_materialized: vec![true; count],
            range_functions: ranges.plan().functions.clone(),
            legality_functions: legality.plan().functions.clone(),
        }
    }

    pub fn range_functions(&self) -> &[Arc<selected_instructions::FunctionLiveRanges>] {
        &self.range_functions
    }

    pub fn legality_functions(&self) -> &[Arc<register_homes::FunctionAllocationLegality>] {
        &self.legality_functions
    }

    /// Recomputes facts only for functions whose selected body changed
    /// between `previous` and `selected`. Changed functions get the liveness
    /// live ranges need and nothing more; `seal` fills in the per-instruction
    /// live sets once, so a caller that reanalyzes many times before sealing
    /// pays for them only at the end.
    pub fn reanalyze_changed_functions(
        &self,
        previous: &impl SelectedProgramView,
        selected: &impl SelectedProgramView,
        availability: &ValidatedAllocatorAvailability,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        reservations: &register_model::ValidatedRegisterReservationProfile,
    ) -> Result<Self, RecoveryReanalysisError> {
        self.reanalyze_functions(
            previous,
            selected,
            availability,
            physical,
            reservations,
            LivenessDetail::RangesOnly,
        )
    }

    fn reanalyze_functions(
        &self,
        previous: &impl SelectedProgramView,
        selected: &impl SelectedProgramView,
        availability: &ValidatedAllocatorAvailability,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        reservations: &register_model::ValidatedRegisterReservationProfile,
        detail: LivenessDetail,
    ) -> Result<Self, RecoveryReanalysisError> {
        if !self.belongs_to(previous)
            || availability.receipt().physical() != physical.identity()
            || availability.receipt().reservations != reservations.identity()
            || selected.selected_plan().target != reservations.target()
        {
            return Err(RecoveryReanalysisError::Legality(
                AllocationLegalityError::RootMismatch,
            ));
        }
        let prev_plan = previous.selected_plan();
        let next_plan = selected.selected_plan();
        let count = next_plan.functions.len();
        let availability_unchanged = self.availability == availability.receipt().identity();
        let mut next = Self {
            selected: selected.selected_identity(),
            availability: availability.receipt().identity(),
            liveness_functions: Vec::with_capacity(count),
            liveness_materialized: Vec::with_capacity(count),
            range_functions: Vec::with_capacity(count),
            legality_functions: Vec::with_capacity(count),
        };
        for (index, function) in next_plan.functions.iter().enumerate() {
            let function_unchanged = prev_plan
                .functions
                .shares_function_storage(&next_plan.functions, index)
                || prev_plan.functions.get(index) == Some(function);
            let (liveness, materialized, ranges) = if function_unchanged {
                (
                    Arc::clone(&self.liveness_functions[index]),
                    self.liveness_materialized[index],
                    Arc::clone(&self.range_functions[index]),
                )
            } else {
                let liveness = match detail {
                    LivenessDetail::RangesOnly => {
                        liveness::compute::compute_function_for_ranges(index, function)
                    }
                    LivenessDetail::Complete => {
                        liveness::compute::compute_function(index, function)
                    }
                }
                .map_err(RecoveryReanalysisError::Liveness)?;
                let ranges = live_ranges::compute::compute_function(index, function, &liveness)
                    .map_err(RecoveryReanalysisError::Ranges)?;
                (
                    Arc::new(liveness),
                    detail == LivenessDetail::Complete,
                    Arc::new(ranges),
                )
            };
            let legality = if function_unchanged && availability_unchanged {
                Arc::clone(&self.legality_functions[index])
            } else {
                let prior = self
                    .range_functions
                    .get(index)
                    .zip(self.legality_functions.get(index))
                    .filter(|_| availability_unchanged)
                    .map(|(ranges, legality)| (ranges.as_ref(), legality.as_ref()));
                let legality = allocation_legality::compute::compute_function_reusing(
                    index,
                    &ranges,
                    availability,
                    physical,
                    reservations,
                    prior,
                )
                .map_err(RecoveryReanalysisError::Legality)?;
                allocation_legality::validate::FunctionCounts::new(&legality, index)
                    .map_err(RecoveryReanalysisError::Legality)?;
                Arc::new(legality)
            };
            next.liveness_functions.push(liveness);
            next.liveness_materialized.push(materialized);
            next.range_functions.push(ranges);
            next.legality_functions.push(legality);
        }
        Ok(next)
    }

    fn belongs_to(&self, selected: &impl SelectedProgramView) -> bool {
        let count = selected.selected_plan().functions.len();
        self.selected == selected.selected_identity()
            && self.liveness_functions.len() == count
            && self.liveness_materialized.len() == count
            && self.range_functions.len() == count
            && self.legality_functions.len() == count
    }

    pub fn seal(
        self,
        anchor_selected: &impl SelectedProgramView,
        anchor_liveness: &ValidatedLiveness,
        anchor_ranges: &ValidatedLiveRanges,
        anchor_legality: &ValidatedAllocationLegality,
        selected: &impl SelectedProgramView,
        availability: &ValidatedAllocatorAvailability,
        register_environment: register_model::TargetRegisterEnvironmentIdentity,
    ) -> Result<
        (
            ValidatedLiveness,
            ValidatedLiveRanges,
            ValidatedAllocationLegality,
        ),
        AllocationLegalityError,
    > {
        if !self.belongs_to(selected)
            || self.availability != availability.receipt().identity()
            || availability.receipt().register_environment() != register_environment
        {
            return Err(AllocationLegalityError::RootMismatch);
        }
        let anchor_plan = anchor_selected.selected_plan();
        let next_plan = selected.selected_plan();
        let mut liveness_functions = self.liveness_functions;
        for (index, materialized) in self.liveness_materialized.into_iter().enumerate() {
            if !materialized {
                let full = liveness::compute::compute_function(index, &next_plan.functions[index])
                    .map_err(|_| AllocationLegalityError::RootMismatch)?;
                liveness_functions[index] = Arc::new(full);
            }
        }
        let carried: Vec<bool> = next_plan
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| {
                (anchor_plan
                    .functions
                    .shares_function_storage(&next_plan.functions, index)
                    || anchor_plan.functions.get(index) == Some(function))
                    && anchor_liveness
                        .plan()
                        .functions
                        .get(index)
                        .is_some_and(|a| {
                            liveness_functions
                                .get(index)
                                .is_some_and(|b| Arc::ptr_eq(a, b) || a == b)
                        })
                    && anchor_ranges.plan().functions.get(index).is_some_and(|a| {
                        self.range_functions
                            .get(index)
                            .is_some_and(|b| Arc::ptr_eq(a, b) || a == b)
                    })
                    && anchor_legality
                        .plan()
                        .functions
                        .get(index)
                        .is_some_and(|a| {
                            self.legality_functions
                                .get(index)
                                .is_some_and(|b| Arc::ptr_eq(a, b) || a == b)
                        })
            })
            .collect();
        let liveness_plan = selected_instructions::LivenessPlan {
            selected: selected.selected_identity(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            target: next_plan.target,
            functions: liveness_functions,
        };
        let validated_liveness = liveness::validate::admit_validated_liveness(
            liveness_plan,
            &carried,
            &anchor_liveness.function_digests,
        );
        let range_plan = selected_instructions::LiveRangePlan {
            selected: selected.selected_identity(),
            liveness: validated_liveness.receipt().identity(),
            optimization_unit: selected.optimization_unit_identity(),
            work_schedule: selected.work_schedule_identity(),
            target: next_plan.target,
            functions: self.range_functions,
        };
        let validated_ranges = live_ranges::validate::admit(
            range_plan,
            &carried,
            &anchor_ranges.function_digests,
            &anchor_ranges.function_counts,
        );
        let legality_plan = register_homes::AllocationLegalityPlan {
            ranges: validated_ranges.receipt().identity(),
            register_environment,
            allocator_availability: availability.receipt().identity(),
            functions: self.legality_functions,
        };
        let validated_legality = allocation_legality::validate::admit(
            legality_plan,
            &carried,
            &anchor_legality.function_digests,
            &anchor_legality.function_counts,
        )?;
        Ok((validated_liveness, validated_ranges, validated_legality))
    }
}

/// How much liveness `UnsealedRecoveryFacts` computes for a changed function.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LivenessDetail {
    /// Only what live ranges read; `seal` adds per-instruction live sets.
    RangesOnly,
    /// The full liveness `analyze_liveness` publishes, so `seal` adds nothing.
    Complete,
}

/// Liveness, live ranges and allocation legality for `selected`, a rewrite of
/// `previous`, given `previous`'s validated facts. Only the functions whose
/// selected body changed are recomputed; every other function keeps its rows,
/// digests and counts. The result equals a fresh `analyze_liveness`,
/// `analyze_live_ranges` and `analyze_allocation_legality` over `selected`.
///
/// A rewrite pass that commits one change at a time and needs sealed facts
/// after each commit uses this instead of reanalyzing the whole program.
pub fn reanalyze_rewritten_functions(
    previous: &impl SelectedProgramView,
    liveness: &ValidatedLiveness,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    selected: &impl SelectedProgramView,
    availability: &ValidatedAllocatorAvailability,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<
    (
        ValidatedLiveness,
        ValidatedLiveRanges,
        ValidatedAllocationLegality,
    ),
    RecoveryReanalysisError,
> {
    UnsealedRecoveryFacts::from_validated(liveness, ranges, legality)
        .reanalyze_functions(
            previous,
            selected,
            availability,
            environment.physical(),
            environment.reservations(),
            LivenessDetail::Complete,
        )?
        .seal(
            previous,
            liveness,
            ranges,
            legality,
            selected,
            availability,
            environment.identity(),
        )
        .map_err(RecoveryReanalysisError::Legality)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn reanalysis_refreshes_legality_after_an_allocator_policy_change() {
        use register_environment::baseline_target_register_environment;
        use register_homes::AllocatorAvailabilityPolicy;
        use semantic_vocabulary::{CallableContractId, ValueId};
        use target::NativeTarget;
        use target_vocabulary::NativeCallTarget;

        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let availability = |policy| {
            materialize_allocator_availability(
                environment.identity(),
                target,
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
                policy,
            )
            .unwrap()
        };
        let all = availability(AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1);
        let mut selected = machine_effects::indirect_call_tests::selected_call(
            &environment,
            target,
            NativeCallTarget::Callable {
                callee: ValueId::new(1).unwrap(),
                contract: CallableContractId::new(1).unwrap(),
            },
        );
        // The indirect target stays live across the fence before its fixed call operand.
        let plan = Arc::make_mut(&mut selected.plan);
        let block = &mut plan.functions[0].blocks[0];
        let fence_constraint = register_model::find_register_constraint(
            &environment.constraints().catalog().constraints,
            keys.atomic_fence.unwrap(),
        )
        .unwrap();
        block.instructions.insert(
            0,
            selected_instructions::SelectedInstruction {
                id: selected_instructions::SelectedInstructionId(2),
                kind: selected_instructions::SelectedInstructionKind::AtomicFence,
                constraint: fence_constraint.key,
                operands: Vec::new(),
                implicit_uses: fence_constraint.implicit_uses.clone(),
                implicit_defs: fence_constraint.implicit_defs.clone(),
                clobbers: fence_constraint.clobbers.clone(),
                provenance: block.instructions[0].provenance.clone(),
            },
        );
        selected.identity = selected_instructions::selected_instruction_plan_identity(plan);
        let liveness = analyze_liveness(&selected).unwrap();
        let ranges = analyze_live_ranges(&selected, &liveness).unwrap();
        let analyze = |availability: &ValidatedAllocatorAvailability| {
            analyze_allocation_legality(
                &ranges,
                availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
            )
        };
        let unrestricted = analyze(&all).unwrap();
        let allowed = unrestricted.plan().functions[0].virtual_registers[0].spans[0].candidates[0];
        let restricted = availability(
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                views: vec![allowed],
            },
        );
        let fresh = analyze(&restricted).unwrap();
        assert_ne!(unrestricted.plan().functions, fresh.plan().functions);
        let cache = UnsealedRecoveryFacts::from_validated(&liveness, &ranges, &unrestricted);
        let seal = |facts: UnsealedRecoveryFacts, availability: &ValidatedAllocatorAvailability| {
            facts.seal(
                &selected,
                &liveness,
                &ranges,
                &unrestricted,
                &selected,
                availability,
                environment.identity(),
            )
        };
        let no_views = availability(
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views: Vec::new() },
        );
        let windows = baseline_target_register_environment(NativeTarget::windows_x64()).unwrap();
        assert!(matches!(
            cache.clone().seal(
                &selected,
                &liveness,
                &ranges,
                &unrestricted,
                &selected,
                &all,
                windows.identity(),
            ),
            Err(AllocationLegalityError::RootMismatch)
        ));
        assert!(matches!(
            cache.reanalyze_changed_functions(
                &selected,
                &selected,
                &all,
                environment.physical(),
                windows.reservations(),
            ),
            Err(RecoveryReanalysisError::Legality(
                AllocationLegalityError::RootMismatch
            ))
        ));
        assert!(matches!(
            analyze(&no_views),
            Err(AllocationLegalityError::NoCandidateViews { .. })
        ));
        assert!(matches!(
            cache.reanalyze_changed_functions(
                &selected,
                &selected,
                &no_views,
                environment.physical(),
                environment.reservations(),
            ),
            Err(RecoveryReanalysisError::Legality(
                AllocationLegalityError::NoCandidateViews { .. }
            ))
        ));
        let refreshed = cache
            .reanalyze_changed_functions(
                &selected,
                &selected,
                &restricted,
                environment.physical(),
                environment.reservations(),
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            &cache.range_functions()[0],
            &refreshed.range_functions()[0]
        ));
        assert_eq!(seal(refreshed, &restricted).unwrap().2.plan(), fresh.plan());
        assert!(matches!(
            seal(cache, &restricted),
            Err(AllocationLegalityError::RootMismatch)
        ));
    }

    #[test]
    fn reanalysis_tracks_added_and_removed_functions_and_seals_only_its_selection() {
        use register_environment::baseline_target_register_environment;
        use register_homes::AllocatorAvailabilityPolicy;
        use selected_instructions::selected_instruction_plan_identity;
        use semantic_vocabulary::MachineId;
        use target::NativeTarget;
        use target_vocabulary::NativeCallTarget;

        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let availability = materialize_allocator_availability(
            environment.identity(),
            target,
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &keys,
            AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
        )
        .unwrap();

        let first = machine_effects::indirect_call_tests::selected_call(
            &environment,
            target,
            NativeCallTarget::Machine(MachineId::new(1).unwrap()),
        );
        let mut two = first.clone();
        let plan = Arc::make_mut(&mut two.plan);
        let mut helper = plan.functions[0].clone();
        helper.machine = MachineId::new(2).unwrap();
        plan.functions.push(helper);
        two.identity = selected_instruction_plan_identity(plan);

        let analyze = |selected: &selected_instructions::SelectedProgram| {
            let liveness = analyze_liveness(selected).unwrap();
            let ranges = analyze_live_ranges(selected, &liveness).unwrap();
            let legality = analyze_allocation_legality(
                &ranges,
                &availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
            )
            .unwrap();
            (liveness, ranges, legality)
        };
        let (liveness, ranges, legality) = analyze(&first);
        let original = UnsealedRecoveryFacts::from_validated(&liveness, &ranges, &legality);
        let reanalyze = |facts: &UnsealedRecoveryFacts,
                         old: &selected_instructions::SelectedProgram,
                         new: &selected_instructions::SelectedProgram| {
            facts
                .reanalyze_changed_functions(
                    old,
                    new,
                    &availability,
                    environment.physical(),
                    environment.reservations(),
                )
                .unwrap()
        };
        let seal = |facts: UnsealedRecoveryFacts,
                    selected: &selected_instructions::SelectedProgram| {
            facts.seal(
                &first,
                &liveness,
                &ranges,
                &legality,
                selected,
                &availability,
                environment.identity(),
            )
        };
        let grown = reanalyze(&original, &first, &two);
        assert!(matches!(
            seal(grown.clone(), &first),
            Err(AllocationLegalityError::RootMismatch)
        ));
        let recovered = seal(grown.clone(), &two).unwrap();
        let fresh = analyze(&two);
        assert_eq!(recovered.0.plan(), fresh.0.plan());
        assert_eq!(recovered.1.plan(), fresh.1.plan());
        assert_eq!(recovered.2.plan(), fresh.2.plan());

        let reduced = reanalyze(&grown, &two, &first);
        let recovered = seal(reduced, &first).unwrap();
        assert_eq!(recovered.0.plan(), liveness.plan());
        assert_eq!(recovered.1.plan(), ranges.plan());
        assert_eq!(recovered.2.plan(), legality.plan());
    }

    #[test]
    fn rewritten_function_reanalysis_equals_a_fresh_analysis_and_carries_the_rest() {
        use register_environment::baseline_target_register_environment;
        use register_homes::AllocatorAvailabilityPolicy;
        use selected_instructions::selected_instruction_plan_identity;
        use semantic_vocabulary::MachineId;
        use target::NativeTarget;
        use target_vocabulary::NativeCallTarget;

        let target = NativeTarget::linux_x64();
        let environment = baseline_target_register_environment(target).unwrap();
        let keys = environment.allocation_constraint_keys();
        let availability = materialize_allocator_availability(
            environment.identity(),
            target,
            environment.physical(),
            environment.constraints(),
            environment.reservations(),
            &keys,
            AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1,
        )
        .unwrap();
        let analyze = |selected: &selected_instructions::SelectedProgram| {
            let liveness = analyze_liveness(selected).unwrap();
            let ranges = analyze_live_ranges(selected, &liveness).unwrap();
            let legality = analyze_allocation_legality(
                &ranges,
                &availability,
                environment.identity(),
                environment.physical(),
                environment.constraints(),
                environment.reservations(),
                &keys,
            )
            .unwrap();
            (liveness, ranges, legality)
        };

        // Two functions; the rewrite inserts a fence into the second only.
        let mut original = machine_effects::indirect_call_tests::selected_call(
            &environment,
            target,
            NativeCallTarget::Machine(MachineId::new(1).unwrap()),
        );
        let plan = Arc::make_mut(&mut original.plan);
        let mut helper = plan.functions[0].clone();
        helper.machine = MachineId::new(2).unwrap();
        plan.functions.push(helper);
        original.identity = selected_instruction_plan_identity(plan);
        let mut rewritten = original.clone();
        let plan = Arc::make_mut(&mut rewritten.plan);
        let fence_constraint = register_model::find_register_constraint(
            &environment.constraints().catalog().constraints,
            keys.atomic_fence.unwrap(),
        )
        .unwrap();
        let block = &mut plan.functions[1].blocks[0];
        block.instructions.insert(
            0,
            selected_instructions::SelectedInstruction {
                id: selected_instructions::SelectedInstructionId(2),
                kind: selected_instructions::SelectedInstructionKind::AtomicFence,
                constraint: fence_constraint.key,
                operands: Vec::new(),
                implicit_uses: fence_constraint.implicit_uses.clone(),
                implicit_defs: fence_constraint.implicit_defs.clone(),
                clobbers: fence_constraint.clobbers.clone(),
                provenance: block.instructions[0].provenance.clone(),
            },
        );
        rewritten.identity = selected_instruction_plan_identity(plan);
        assert!(
            original
                .plan
                .functions
                .shares_function_storage(&rewritten.plan.functions, 0)
        );

        let (liveness, ranges, legality) = analyze(&original);
        let reanalyzed = reanalyze_rewritten_functions(
            &original,
            &liveness,
            &ranges,
            &legality,
            &rewritten,
            &availability,
            &environment,
        )
        .unwrap();
        let fresh = analyze(&rewritten);
        assert_eq!(reanalyzed.0, fresh.0);
        assert_eq!(reanalyzed.1, fresh.1);
        assert_eq!(reanalyzed.2, fresh.2);
        assert_ne!(
            reanalyzed.0.plan().functions[1],
            liveness.plan().functions[1]
        );
        assert!(Arc::ptr_eq(
            &reanalyzed.0.plan().functions[0],
            &liveness.plan().functions[0]
        ));
        assert!(Arc::ptr_eq(
            &reanalyzed.2.plan().functions[0],
            &legality.plan().functions[0]
        ));

        // Facts that do not describe `previous` are refused, not reused.
        assert!(matches!(
            reanalyze_rewritten_functions(
                &rewritten,
                &liveness,
                &ranges,
                &legality,
                &original,
                &availability,
                &environment,
            ),
            Err(RecoveryReanalysisError::Legality(
                AllocationLegalityError::RootMismatch
            ))
        ));
    }
}

#[cfg(test)]
mod report_counts_tests {

    use register_homes::{
        AllocationLegalityPlan, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
        VirtualLegalitySpan, VirtualRegisterAllocationLegality,
    };
    use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
    use selected_instructions::{
        FunctionLiveRanges, LiveRangeFragment, LiveRangeIdentity, LiveRangePlan, LiveRangePoint,
        LivenessIdentity, SelectedBlockId, SelectedInstructionPlanIdentity, VirtualLiveRange,
        VirtualRegisterId,
    };
    use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
    use std::sync::Arc;

    #[test]
    fn range_receipt_counts_match_fresh_projection_after_partial_carry() {
        let register = VirtualLiveRange {
            virtual_register: VirtualRegisterId(0),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![LiveRangeFragment {
                block: SelectedBlockId(0),
                start: LiveRangePoint(1),
                end: LiveRangePoint(4),
            }],
            edge_connectors: Vec::new(),
        };
        let function = |machine| {
            Arc::new(FunctionLiveRanges {
                machine: MachineId::new(machine).unwrap(),
                block_domains: Vec::new(),
                virtual_registers: vec![register.clone()],
                tied_pairs: Vec::new(),
                edge_transfers: Vec::new(),
                copy_affinities: Vec::new(),
                early_clobbers: Vec::new(),
                architectural_units: Vec::new(),
                interference: Default::default(),
            })
        };
        let mut plan = LiveRangePlan {
            selected: SelectedInstructionPlanIdentity::from_canonical_bytes(b"counts"),
            liveness: LivenessIdentity::from_bytes([1; 32]),
            optimization_unit: optimization_core::OptimizationUnitIdentity::from_bytes([2; 32]),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            target: target::NativeTarget::macos_arm64(),
            functions: vec![function(1), function(2)],
        };
        let previous = super::live_ranges::validate::admit(plan.clone(), &[], &[], &[]);
        Arc::make_mut(&mut plan.functions[1])
            .virtual_registers
            .push(VirtualLiveRange {
                virtual_register: VirtualRegisterId(1),
                ..register
            });
        let fresh = super::live_ranges::validate::admit(plan.clone(), &[], &[], &[]);
        assert_eq!(fresh.receipt().virtual_register_count(), 3);
        assert_eq!(fresh.receipt().virtual_fragment_count(), 3);
        for previous_counts in [&previous.function_counts[..], &[]] {
            let carried = super::live_ranges::validate::admit(
                plan.clone(),
                &[true, false],
                &previous.function_digests,
                previous_counts,
            );
            assert_eq!(carried, fresh);
        }
    }

    #[test]
    fn legality_receipt_counts_match_fresh_projection_after_partial_carry() {
        let register = VirtualRegisterAllocationLegality {
            virtual_register: VirtualRegisterId(0),
            class: RegisterClassId(0),
            spans: vec![VirtualLegalitySpan {
                block: SelectedBlockId(0),
                start: LiveRangePoint(1),
                end: LiveRangePoint(4),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            early_clobber_points: Vec::new(),
            entry_transitions: Vec::new(),
        };
        let function = |machine| {
            Arc::new(FunctionAllocationLegality {
                machine: MachineId::new(machine).unwrap(),
                virtual_registers: vec![register.clone()],
            })
        };
        let mut plan = AllocationLegalityPlan {
            ranges: LiveRangeIdentity::from_bytes([1; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([2; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([3; 32]),
            functions: vec![function(1), function(2)],
        };
        let previous =
            super::allocation_legality::validate::admit(plan.clone(), &[], &[], &[]).unwrap();
        let changed = &mut Arc::make_mut(&mut plan.functions[1]).virtual_registers[0];
        changed.spans[0].end = LiveRangePoint(8);
        changed.spans[0].candidates.push(RegisterViewId(2));
        let fresh =
            super::allocation_legality::validate::admit(plan.clone(), &[], &[], &[]).unwrap();
        assert_eq!(fresh.receipt().point_count(), 10);
        assert_eq!(fresh.receipt().candidate_count(), 27);
        for previous_counts in [&previous.function_counts[..], &[]] {
            let carried = super::allocation_legality::validate::admit(
                plan.clone(),
                &[true, false],
                &previous.function_digests,
                previous_counts,
            )
            .unwrap();
            assert_eq!(carried, fresh);
        }
    }
}
