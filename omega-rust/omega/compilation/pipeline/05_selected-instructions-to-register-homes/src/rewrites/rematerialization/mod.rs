//! Optimizer module role: executable entrance. Active-resident pressure-rematerialization stage.
//!
//! The producer rebuilds all allocation facts from the transformed selected
//! CFG. This entrance grants stage custody only after independent replay
//! validation reconstructs that complete chain.

mod compute;
mod custody;

use optimization_core::OptimizationWorkBudget;

use crate::{
    RegisterHomeError, ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use register_homes::{
    AllocationLegalityIdentity, PostAllocationOptimizationManifestError,
    RecoveryClassificationIdentity, RecoveryClassificationPolicy, SpillChoiceIdentity,
    SpillChoicePolicy,
};
use selected_instructions::{
    LiveRangeIdentity, LivenessIdentity, PressureRematerializationIdentity,
};
use selected_instructions_to_selected_instructions::{
    AllocationLegalityError, LiveRangeError, LivenessError, PressureRematerializationError,
    PressureRematerializationPolicy, RecoveryClassificationError, SpillChoiceError,
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPressureRematerialization,
};

pub(crate) use compute::{
    complete_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_pressure,
};

/// One bounded active-resident rematerialization sweep followed by analyses,
/// homes, and a typed post-allocation manifest rebuilt from the transformed
/// selected CFG. The source analyses remain retained only as input custody.
#[derive(Debug)]
pub(crate) struct StagedOptimizedActiveResidentRematerialization {
    source: StagedOptimizedAllocationLegality,
    rematerialization: ValidatedPressureRematerialization,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerialization {
    /// The retained legality stage the proven sweep consumed. Replay and
    /// custody validation inspect it; ordinary consumers read the current
    /// program and rebuilt analyses directly.
    /// The register environment admitted with the retained source — the sweep
    /// preserves it, so the source's environment is the current one.
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.source.register_environment()
    }
    /// The governing optimizer selections admitted with the retained stage.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.source.selections()
    }
    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.source.budget_per_pass()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.source
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    receipt_accessor_rows!(borrow StagedOptimizedActiveResidentRematerialization {
        rematerialization: &ValidatedPressureRematerialization,
        liveness: &ValidatedLiveness,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
        homes: &ValidatedRegisterHomes,
    });
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    receipt_accessor_rows!(ref StagedOptimizedActiveResidentRematerialization {
        custody: StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    });
}

/// Custody receipt for the completed sweep: the proven rematerialization
/// prefix the pressure stage already minted, plus the terminal homes
/// assignment and the typed manifest that assignment rebuilt. The prefix
/// receipt is carried whole, so no stage identity is projected twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    prefix: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    homes: crate::RegisterHomeIdentity,
    manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
    assignment_count: usize,
}

/// Re-serve a receipt's rows from the prefix receipt it carries: one internal
/// rule per accessor, so the completion names the prefix field instead of
/// restating each delegation body.
macro_rules! prefix_accessor_rows {
    ($prefix:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { self.$prefix.$field } )*
    };
}

impl StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    prefix_accessor_rows!(prefix {
        source: StagedOptimizedAllocationLegalityCustodyReceipt,
        choices: SpillChoiceIdentity,
        choice_policy: SpillChoicePolicy,
        choice_usage: optimization_core::OptimizationWorkUsage,
        classifications: RecoveryClassificationIdentity,
        classification_policy: RecoveryClassificationPolicy,
        classification_usage: optimization_core::OptimizationWorkUsage,
        rematerialization: PressureRematerializationIdentity,
        rematerialization_policy: PressureRematerializationPolicy,
        rematerialization_usage: optimization_core::OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
        transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
        ranges: LiveRangeIdentity,
        legality: AllocationLegalityIdentity,
        function_count: usize,
        virtual_register_count: usize,
        applied_count: usize,
        rewritten_use_count: usize,
    });
    receipt_accessor_rows!(
        StagedOptimizedActiveResidentRematerializationCustodyReceipt {
            homes: crate::RegisterHomeIdentity,
            manifest: optimization_core::PostAllocationOptimizationManifestIdentity,
            assignment_count: usize,
        }
    );
}

/// The recorded sweep once its applied rewrite and rebuilt facts are proven
/// but before terminal homes assignment is attempted. Residual
/// `NoCompatibleHome` pressure hands this custody to runtime-spill recovery,
/// which keeps the rematerialization as the first recorded transformation
/// and independently replays this whole prefix before any spill step.
#[derive(Debug)]
pub(crate) struct StagedOptimizedActiveResidentRematerializationPressure {
    source: StagedOptimizedAllocationLegality,
    rematerialization: ValidatedPressureRematerialization,
    liveness: ValidatedLiveness,
    ranges: ValidatedLiveRanges,
    legality: ValidatedAllocationLegality,
    custody: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
}

impl StagedOptimizedActiveResidentRematerializationPressure {
    receipt_accessor_rows!(borrow StagedOptimizedActiveResidentRematerializationPressure {
        source: &StagedOptimizedAllocationLegality,
        rematerialization: &ValidatedPressureRematerialization,
        liveness: &ValidatedLiveness,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
    });
    receipt_accessor_rows!(ref StagedOptimizedActiveResidentRematerializationPressure {
        custody: StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    });
}

/// Custody receipt for the proven rematerialization prefix: every identity
/// the terminal or composing completion must reproduce, without homes or
/// manifest — those belong to whichever path resolves the residual pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: SpillChoiceIdentity,
    choice_policy: SpillChoicePolicy,
    choice_usage: optimization_core::OptimizationWorkUsage,
    classifications: RecoveryClassificationIdentity,
    classification_policy: RecoveryClassificationPolicy,
    classification_usage: optimization_core::OptimizationWorkUsage,
    rematerialization: PressureRematerializationIdentity,
    rematerialization_policy: PressureRematerializationPolicy,
    rematerialization_usage: optimization_core::OptimizationWorkUsage,
    budget: OptimizationWorkBudget,
    transformed_selected: selected_instructions::SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    ranges: LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    function_count: usize,
    virtual_register_count: usize,
    applied_count: usize,
    rewritten_use_count: usize,
}

impl StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
    receipt_accessor_rows!(
        StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt {
            source: StagedOptimizedAllocationLegalityCustodyReceipt,
            rematerialization: PressureRematerializationIdentity,
        }
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedActiveResidentRematerializationError {
    UnsupportedPolicy,
    SpillChoice(SpillChoiceError),
    Classification(RecoveryClassificationError),
    Rematerialization(PressureRematerializationError),
    NoAppliedAction,
    Liveness(LivenessError),
    Ranges(LiveRangeError),
    Legality(AllocationLegalityError),
    RemainingTransitions { count: usize },
    Homes(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedActiveResidentRematerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized active-resident rematerialization failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedActiveResidentRematerializationError {}
