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
  Selected optimization (`04_selected-instructions-to-selected-instructions`)
  and register allocation (`05_selected-instructions-to-register-homes`) both
  require authenticated liveness, live-range, and legality facts over the
  same `SelectedInstructionPlan` representation. Housing these analyses in a
  single representation-analysis crate lets both pipeline stages share the
  exact dataflow and interference engines, while `UnsealedRecoveryFacts` and
  `carried_digests` reuse unchanged per-function `Arc` rows across iterative
  spill/split recovery steps without re-analyzing or re-hashing untouched
  functions.
*/

/*
  Consumes `SelectedProgram`, `SelectedInstructionPlan`, and target register
  models (`ValidatedTargetRegisterEnvironment`, `ValidatedPhysicalRegisterModel`)
  through `SelectedProgramView`. Produces sealed analysis carriers and custody
  receipts (`StagedOptimizedLiveness`, `StagedOptimizedLiveRanges`,
  `StagedOptimizedAllocationLegality`, `SelectedOptimizationOutput`) consumed by
  `04_selected-instructions-to-selected-instructions` and
  `05_selected-instructions-to-register-homes`.
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

    use crate::rewriting::RegisterAllocationRuleTargetApplicability;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AllocationRecoveryRuleCatalogPayload {
        target: RegisterAllocationRuleTargetApplicability,
    }

    impl AllocationRecoveryRuleCatalogPayload {}

    pub type AllocationRecoveryRuleCatalogEntry =
        OptimizationCatalogDescriptor<AllocationRecoveryRuleCatalogPayload>;

    pub const ALLOCATION_RECOVERY_RULE_CATALOG: [AllocationRecoveryRuleCatalogEntry; 2] = [
        AllocationRecoveryRuleCatalogEntry::new(
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            AllocationRecoveryRuleCatalogPayload {
                target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            },
        ),
        AllocationRecoveryRuleCatalogEntry::new(
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            AllocationRecoveryRuleCatalogPayload {
                target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            },
        ),
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AllocationRecoveryRuleCatalogError {
        WrongPhase(OptimizationPhaseMismatch),
        UnsupportedSelection(Optimization),
        UnsupportedComposition,
    }

    pub fn selected_allocation_recovery_rule(
        selections: &OptimizationPhaseSelections,
    ) -> Result<Option<Optimization>, AllocationRecoveryRuleCatalogError> {
        let phase = selections
            .require_phase(OptimizationExecutionPhase::AllocationRecovery)
            .map_err(AllocationRecoveryRuleCatalogError::WrongPhase)?;
        match phase.as_slice() {
            [] => Ok(None),
            [selected]
                if ALLOCATION_RECOVERY_RULE_CATALOG
                    .iter()
                    .any(|entry| entry.optimization() == *selected) =>
            {
                Ok(Some(*selected))
            }
            [unsupported] => Err(AllocationRecoveryRuleCatalogError::UnsupportedSelection(
                *unsupported,
            )),
            _ => Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition),
        }
    }
}
pub use allocation_recovery_catalog::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    selected_allocation_recovery_rule,
};
pub mod codec_bytes {

    pub use optimization_core::codec_bytes::push_encoded;
    pub fn encode_header(buffer: &mut Vec<u8>, magic: &[u8; 8], version: u32) {
        buffer.extend_from_slice(magic);
        push_encoded(buffer, version.to_le_bytes());
    }

    pub fn decode_header<E>(
        cursor: &mut crate::cursor::Cursor<'_>,
        magic: &[u8; 8],
        expected_version: u32,
        wrong_magic: impl FnOnce() -> E,
        unsupported_version: impl FnOnce(u32) -> E,
    ) -> Result<(), E>
    where
        E: From<crate::cursor::Truncated> + From<crate::cursor::TakeError>,
    {
        if cursor.take(magic.len())? != magic.as_slice() {
            return Err(wrong_magic());
        }
        let version = cursor.u32()?;
        if version != expected_version {
            return Err(unsupported_version(version));
        }
        Ok(())
    }
}
mod selected_optimization_output {

    use optimization_core::{
        OptimizationSelectionIdentity, PreAllocationOptimizationCompletionIdentity,
        SelectedLoweringOptimizationCompletionIdentity,
    };
    use register_homes::PostAllocationSelectedTransformation;

    use crate::{
        OwnedSelectedProgram, SelectedProgramRef, StagedOptimizedAllocationLegality,
        StagedOptimizedAllocationLegalityCustodyReceipt, StagedOptimizedLiveRanges,
        ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    };

    #[derive(Debug)]
    pub enum SelectedOptimizationOutput {
        Identity(StagedOptimizedLiveRanges),
        SelectedLowering(
            CompletedSelectedOptimization<SelectedLoweringOptimizationCompletionIdentity>,
        ),
        PreAllocation(CompletedSelectedOptimization<PreAllocationOptimizationCompletionIdentity>),
    }

    #[derive(Debug)]
    pub struct CompletedSelectedOptimization<Completion> {
        pub source: StagedOptimizedAllocationLegality,
        pub current: OwnedSelectedProgram,
        pub liveness: ValidatedLiveness,
        pub ranges: ValidatedLiveRanges,
        pub legality: ValidatedAllocationLegality,
        pub ledger: SelectedOptimizationLedger<Completion>,
    }

    impl<Completion> CompletedSelectedOptimization<Completion> {
        pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
            &self.source
        }

        pub fn current(&self) -> SelectedProgramRef<'_> {
            SelectedProgramRef::new(&self.current)
        }

        pub const fn liveness(&self) -> &ValidatedLiveness {
            &self.liveness
        }

        pub const fn ranges(&self) -> &ValidatedLiveRanges {
            &self.ranges
        }

        pub const fn legality(&self) -> &ValidatedAllocationLegality {
            &self.legality
        }

        pub const fn ledger(&self) -> &SelectedOptimizationLedger<Completion> {
            &self.ledger
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedOptimizationLedger<Completion> {
        pub identity: Completion,
        pub phase_selections: OptimizationSelectionIdentity,
        pub source: StagedOptimizedAllocationLegalityCustodyReceipt,
        pub transformations: Vec<PostAllocationSelectedTransformation>,
    }

    impl<Completion: Copy> SelectedOptimizationLedger<Completion> {
        pub const fn identity(&self) -> Completion {
            self.identity
        }

        pub const fn phase_selections(&self) -> OptimizationSelectionIdentity {
            self.phase_selections
        }

        pub const fn source(&self) -> &StagedOptimizedAllocationLegalityCustodyReceipt {
            &self.source
        }

        pub fn transformations(&self) -> &[PostAllocationSelectedTransformation] {
            &self.transformations
        }
    }
}
pub use selected_optimization_output::{
    CompletedSelectedOptimization, SelectedOptimizationLedger, SelectedOptimizationOutput,
};
pub mod cursor {

    pub use optimization_core::cursor::{TakeError, Truncated};
    pub struct Cursor<'a> {
        encoded: &'a [u8],
        offset: usize,
    }

    impl<'a> Cursor<'a> {
        pub const fn new(encoded: &'a [u8]) -> Self {
            Self { encoded, offset: 0 }
        }

        pub fn remaining(&self) -> usize {
            self.encoded.len().saturating_sub(self.offset)
        }

        pub fn take(&mut self, count: usize) -> Result<&'a [u8], Truncated> {
            let end = self.offset.checked_add(count).ok_or(Truncated)?;
            let value = self.encoded.get(self.offset..end).ok_or(Truncated)?;
            self.offset = end;
            Ok(value)
        }

        pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
            self.take(N)?.try_into().map_err(|_| Truncated)
        }

        pub fn byte(&mut self) -> Result<u8, Truncated> {
            Ok(self.array::<1>()?[0])
        }

        pub fn length(&mut self) -> Result<usize, TakeError> {
            let value = u64::from_le_bytes(self.array().map_err(|_| TakeError::Truncated)?);
            usize::try_from(value).map_err(|_| TakeError::LengthOverflow)
        }

        pub fn u16(&mut self) -> Result<u16, Truncated> {
            Ok(u16::from_le_bytes(self.array()?))
        }

        pub fn u32(&mut self) -> Result<u32, Truncated> {
            Ok(u32::from_le_bytes(self.array()?))
        }

        pub fn u64(&mut self) -> Result<u64, Truncated> {
            Ok(u64::from_le_bytes(self.array()?))
        }

        pub fn u128(&mut self) -> Result<u128, Truncated> {
            Ok(u128::from_le_bytes(self.array()?))
        }
    }
}
pub mod rewriting;
pub mod work_accounting;
pub use work_accounting::WorkDefect;
mod optimization_input {

    use optimization_core::{OptimizationSelections, OptimizationWorkBudget};
    use register_environment::ValidatedTargetRegisterEnvironment;
    use selected_instructions::{SelectedProgram, StagedOptimizedCustodyCore};

    #[derive(Debug)]
    pub struct SelectedOptimizationInput {
        pub program: SelectedProgram,
        pub custody: StagedOptimizedCustodyCore,
        pub register_environment: ValidatedTargetRegisterEnvironment,
        pub selections: OptimizationSelections,
        pub budget_per_pass: OptimizationWorkBudget,
    }

    impl SelectedOptimizationInput {
        pub const fn selected(&self) -> &SelectedProgram {
            &self.program
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

pub use legality::stage_optimized_allocation_legality_for_frameless_leaf;
pub use legality::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
};
pub use live_ranges::{
    LiveRangeError, StagedOptimizedLiveRanges, ValidatedLiveRanges, analyze_live_ranges,
    analyze_live_ranges_reusing,
};
pub use live_ranges::{
    LiveRangeValidationReceipt, OptimizedLiveRangeCustodyError,
    StagedOptimizedLiveRangeCustodyReceipt, stage_optimized_live_ranges,
};
pub use liveness::{LivenessError, ValidatedLiveness, analyze_liveness, analyze_liveness_reusing};
pub use liveness::{
    LivenessValidationReceipt, OptimizedLivenessCustodyError, StagedOptimizedLiveness,
    StagedOptimizedLivenessCustodyReceipt, stage_optimized_liveness,
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
}
pub mod spill_choice;
pub use allocation_legality::AllocationLegalityValidationReceipt;
pub use allocation_legality::{
    AllocationLegalityError, ValidatedAllocationLegality, analyze_allocation_legality,
    analyze_allocation_legality_reusing,
};
pub use allocator_availability::AllocatorAvailabilityError;
pub use allocator_availability::ValidatedAllocatorAvailability;
pub use allocator_availability::materialize_allocator_availability;
pub use fixed_precolored_intervals::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    ValidatedFixedPrecoloredIntervals, analyze_fixed_precolored_intervals,
};
pub use fixed_precolored_segment_homes::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    ValidatedFixedPrecoloredSegmentHomes, assign_fixed_precolored_segment_homes,
};
pub use fixed_precolored_split_requirements::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredSplitRequirements, analyze_fixed_precolored_split_requirements,
};
pub use recovery_classification::{
    RecoveryClassificationError, ValidatedRecoveryClassifications, classify_pressure_recovery,
};
pub use selected_input::{OwnedSelectedProgram, SelectedProgramRef, SelectedProgramView};

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

    pub fn reanalyze_changed_functions(
        &self,
        previous: &impl SelectedProgramView,
        selected: &impl SelectedProgramView,
        availability: &ValidatedAllocatorAvailability,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        reservations: &register_model::ValidatedRegisterReservationProfile,
    ) -> Result<Self, RecoveryReanalysisError> {
        let prev_plan = previous.selected_plan();
        let next_plan = selected.selected_plan();
        let count = next_plan.functions.len();
        let mut changed = vec![false; count];
        for (index, function) in next_plan.functions.iter().enumerate() {
            if !(prev_plan
                .functions
                .shares_function_storage(&next_plan.functions, index)
                || prev_plan.functions.get(index) == Some(function))
            {
                changed[index] = true;
            }
        }
        let mut liveness_functions = self.liveness_functions.clone();
        let mut liveness_materialized = self.liveness_materialized.clone();
        if liveness_materialized.len() < count {
            liveness_materialized.resize(count, false);
        }
        for (index, function) in next_plan.functions.iter().enumerate() {
            if changed[index] {
                liveness_functions[index] = Arc::new(
                    liveness::compute::compute_function_for_ranges(index, function)
                        .map_err(RecoveryReanalysisError::Liveness)?,
                );
                liveness_materialized[index] = false;
            }
        }
        let mut range_functions = self.range_functions.clone();
        for (index, function) in next_plan.functions.iter().enumerate() {
            if changed[index] {
                range_functions[index] = Arc::new(
                    live_ranges::compute::compute_function(
                        index,
                        function,
                        &liveness_functions[index],
                    )
                    .map_err(RecoveryReanalysisError::Ranges)?,
                );
            }
        }
        let mut legality_functions = self.legality_functions.clone();
        for index in 0..count {
            if changed[index] {
                let prior_pair = self
                    .range_functions
                    .get(index)
                    .zip(self.legality_functions.get(index))
                    .map(|(r, l)| (r.as_ref(), l.as_ref()));
                let computed = allocation_legality::compute::compute_function_reusing(
                    index,
                    &range_functions[index],
                    availability,
                    physical,
                    reservations,
                    prior_pair,
                )
                .map_err(RecoveryReanalysisError::Legality)?;
                allocation_legality::validate::FunctionCounts::new(&computed, index)
                    .map_err(RecoveryReanalysisError::Legality)?;
                legality_functions[index] = Arc::new(computed);
            }
        }
        Ok(Self {
            liveness_functions,
            liveness_materialized,
            range_functions,
            legality_functions,
        })
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
        let anchor_plan = anchor_selected.selected_plan();
        let next_plan = selected.selected_plan();
        let mut liveness_functions = self.liveness_functions;
        for (index, materialized) in self.liveness_materialized.iter().copied().enumerate() {
            if !materialized
                && let Some(function) = next_plan.functions.get(index)
                && let Ok(full) = liveness::compute::compute_function(index, function)
            {
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
