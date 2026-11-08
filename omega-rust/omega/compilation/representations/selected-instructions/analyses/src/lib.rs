#![forbid(unsafe_code)]

//! Current selected-program facts and their independent validation.
//!
//! Liveness, live ranges, allocation legality, allocator availability,
//! fixed-precolored intervals, segment homes and split requirements, machine
//! effects, recovery classification and spill choice are computed over a
//! selected program and sealed with their receipts. Selected optimization
//! (Omega 04) and register allocation (Omega 05) both run them, so they belong
//! to the selected-instruction family rather than to either stage. The staged
//! carriers (`StagedOptimizedLiveness`, `StagedOptimizedLiveRanges`,
//! `StagedOptimizedAllocationLegality`) chain a [`SelectedOptimizationInput`]
//! through those facts. Bounded work is charged through [`work_accounting`].
//!
//! Inputs are trusted same-compile plans read through [`SelectedProgramView`].
//! This crate does not re-validate a plan it is handed (verification placement
//! in `omega-rust/pipeline.md`); the seal over which programs may be analyzed
//! is enforced where those plans are produced, by each stage's own
//! `ValidatedSelectedAnalysis`.

/// The six analysis identities every validation receipt and analysis input
/// in this stage carries: live ranges, allocation legality, the register
/// environment, allocator availability, the optimization unit and the fuel
/// schedule. Expanded inside each receipt's `impl` so `self.<field>`
/// resolves on the receiver's own field names.
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
        $visibility const fn fuel_schedule(self) -> FuelScheduleIdentity {
            self.fuel_schedule
        }
    };
}

/// One owner row per receipt accessor: the field's own name read back as its
/// own type. A receipt whose rows are not one of the shared identity blocks
/// lists them here instead of restating the body at the site; `ref` selects
/// the `&self` receiver for receipts whose callers hold a reference.
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

/// A by-value receipt whose accessor rows are exactly its fields. The struct
/// and `receipt_accessor_rows!` stay one roster.
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

/// The producer rows a staged shell re-serves unchanged: one internal rule per
/// accessor, so a shell names its producer field and the rows it re-serves
/// instead of restating each delegation body and its doc. `$producer` is the
/// field holding the retained producer stage, and every row delegates to that
/// producer's own method of the same name.
#[macro_export]
macro_rules! staged_producer_accessors {
    ($producer:ident; $($row:ident),* $(,)?) => {
        $( staged_producer_accessors!(@row $producer, $row); )*
    };
    (@row $producer:ident, selected) => {
        /// The current selected program this stage describes.
        pub const fn selected(&self) -> &SelectedProgram {
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

/// `From<WorkDefect>` for every bounded stage error that carries
/// `Work(WorkDefect)`: a charge's `?` lands in the stage's own refusal surface
/// without a per-stage adapter block or a per-site mapping.
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

/// One owner row for a validated analysis carrier: the admitted plan, its
/// receipt, and the two projections every consumer reads — `plan()` and the
/// receipt's own accessor rows. Each analysis names its carrier, plan and
/// receipt types instead of restating the shell.
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

/// The same carrier for an analysis that shares its admitted plan with a
/// successor pass: the plan rides behind an `Arc`, and beside the receipt it
/// keeps each function's share of the receipt identity so a successor plan
/// that carries a function over unchanged need not hash it again. The row
/// names the report-only fields the analysis keeps beside those shares.
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
            /// Each function's share of the receipt identity, kept so a
            /// successor plan that carries a function over unchanged need
            /// not hash it again.
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

/// The whole seal of a freshly produced plan: every receipt row is copied from
/// the plan root the producer just built, and that plan becomes the validated
/// carrier. A bounded pass names its plan, carrier, receipt, identity function
/// and the plan fields its receipt records instead of restating the body.
#[macro_export]
macro_rules! seal_plan {
    (
        $seal:ident($plan:ty) -> $carrier:ident : $receipt:ident
            = $identity:path, $error:ty
        { $($field:ident),* $(,)? }
    ) => {
        /// Admit a freshly produced plan with its receipt; the producer calls
        /// this instead of replaying its own output.
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

/// `Display` and `Error` for the crate's `<context>: {self:?}` refusal
/// convention. The caller passes the whole context, so both the
/// `invalid <rewrite>` and the `<analysis> failed` spellings render from one
/// owner.
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

/// Every validated rewrite carries the same two fields — the transformed plan
/// shared by later stages and the receipt the rewrite replayed — and exposes
/// the same three accessors over them: the plan borrowed and shared, and the
/// receipt. One declarative row per validated type expands the carrier and
/// that impl, so a family names its receipt instead of restating the shell.
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

/// Every selected-rewrite receipt exposes the same four identities: the
/// selected plan before and after, plus the optimization-unit and fuel
/// identities the rewrite carried. One declarative row per receipt expands
/// those accessors; receipts with extra accessors keep them in their own
/// impl.
#[macro_export]
macro_rules! receipt_selected_identities {
    ($name:ident) => {
        impl $name {
            /// Identity of the selected plan the rewrite consumed.
            pub const fn source_selected(&self) -> SelectedInstructionPlanIdentity {
                self.source_selected
            }
            /// Identity of the selected plan the rewrite produced.
            pub const fn transformed_selected(&self) -> SelectedInstructionPlanIdentity {
                self.transformed_selected
            }
            /// Optimization unit the rewrite's evidence belongs to.
            pub const fn optimization_unit(&self) -> OptimizationUnitIdentity {
                self.optimization_unit
            }
            /// Fuel schedule the rewrite ran under.
            pub const fn fuel_schedule(&self) -> FuelScheduleIdentity {
                self.fuel_schedule
            }
        }
    };
}

/// Every selected-rewrite receipt records the same four shared identities and
/// the coordinate rows its transformation names. One declarative row per
/// family expands the structure, the coordinate's own accessor rows and the
/// shared identity accessors, so a family states the rows that distinguish its
/// receipt — with the prose that says which instruction it folded — instead of
/// restating the shell around them.
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
            fuel_schedule: FuelScheduleIdentity,
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
            fuel_schedule: FuelScheduleIdentity,
        }

        receipt_selected_identities!($receipt);
    };
}

/// Every rewrite receipt's canonical identity: the family's own domain
/// separator, both selected-plan identities, the optimization unit, the fuel
/// schedule, the transformed function coordinate, and the rule's instruction
/// coordinate(s). One declarative row per rule expands the encoder and the
/// receipt's own `identity()` accessor, so a rule declares only what
/// distinguishes its identity. A row's prose stays a plain `//` comment: a doc
/// comment on a macro invocation is an unused attribute.
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
                    $crate::codec_bytes::push_encoded(bytes, receipt.fuel_schedule.marker().to_le_bytes());
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
            /// The durable transformation identity the post-allocation
            /// manifest ledger records: the receipt's exact fields under its
            /// family's domain separator.
            pub fn identity(&self) -> $identity {
                $function(self)
            }
        }
    };
}

mod allocation_recovery_catalog;
pub use allocation_recovery_catalog::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    selected_allocation_recovery_rule,
};
pub mod codec_bytes;
mod selected_optimization_output;
pub use selected_optimization_output::{
    CompletedSelectedOptimization, SelectedOptimizationLedger, SelectedOptimizationOutput,
};
pub mod cursor;
pub mod rewriting;
pub mod work_accounting;
pub use work_accounting::WorkDefect;
mod optimization_input;
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
pub use machine_effects::validated_machine_effect_catalog;
pub use machine_effects::{
    MachineEffectStageError, ValidatedPreAllocationMachineEffects, analyze_machine_effects,
};

pub mod allocation_legality;
pub mod allocator_availability;
pub mod fixed_precolored_intervals;
mod fixed_precolored_segment_homes;
pub mod fixed_precolored_split_requirements;
pub mod recovery_classification;
mod selected_input;
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

// The five bounded analyses carry the shared work refusal.
work_defect_conversion!(
    FixedPrecoloredIntervalError,
    FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementError,
    RecoveryClassificationError,
    SpillChoiceError,
);

pub use spill_choice::{SpillChoiceError, ValidatedSpillChoices, choose_spill_victims};
use std::sync::Arc;

/// Each function's digest for the receipt identity: a carried function's
/// comes from its validated predecessor, whose rows it copies; every other
/// function is hashed afresh.
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

/// Intermediate per-function analysis rows during runtime-spill recovery,
/// deferring whole-plan receipt hashing until placement succeeds.
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
            fuel_schedule: selected.fuel_schedule_identity(),
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
            fuel_schedule: selected.fuel_schedule_identity(),
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
mod report_counts_tests;
