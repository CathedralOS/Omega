//! Optimizer module role: executable entrance. Register-home assignment.
//!
//! Baseline legality and post-copy reanalysis are distinct source families;
//! each constructs its home/manifest pair once.

mod construction;
mod custody;

use crate::{
    RegisterHomeError, RegisterHomeIdentity, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes,
};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity,
    PostAllocationOptimizationManifestError,
};
use selected_instructions::{LiveRangeIdentity, LivenessIdentity, SelectedInstructionPlanIdentity};
use selected_instructions_to_selected_instructions::{
    OptimizedSelectedReanalysisError, StagedOptimizedAllocationLegality,
    StagedOptimizedSelectedReanalysis, StagedOptimizedSelectedReanalysisCustodyReceipt,
    ValidatedAllocationLegality, ValidatedFixedViewCopies, ValidatedLiveRanges, ValidatedLiveness,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target_operations_to_selected_instructions::StagedOptimizedCustodyCore;
use terminal_psi::TerminalPsiIdentity;

use super::PostHomeCustodyReceipt;

pub(crate) fn stage_register_homes_with_assignment(
    legality: StagedOptimizedAllocationLegality,
    homes: crate::ValidatedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let staged = construction::construct_with_assignment(legality, homes)?;
    Ok(staged)
}

/// Post-copy assignment probe the recovery route runs while it still owns the
/// reanalysis, so residual `NoCompatibleHome` pressure can be answered by
/// runtime spill instead of failing the fixed-view sequence.
pub(crate) fn assign_optimized_register_homes_after_fixed_view_copies(
    reanalysis: &StagedOptimizedSelectedReanalysis,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    construction::assign_optimized_register_homes_after_fixed_view_copies(reanalysis)
}

pub(crate) fn stage_register_homes_after_fixed_view_copies_with_assignment(
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let staged =
        construction::construct_optimized_register_homes_after_fixed_view_copies_with_assignment(
            reanalysis, homes,
        )?;
    Ok(staged)
}

/// Bounded opt-in physical-home staging. This lane admits only legality plans
/// with at least one shared legal candidate per VReg and no unresolved
/// fixed-view transition or spill requirement. It grants no machine-emission
/// or publication authority.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomes {
    legality: StagedOptimizedAllocationLegality,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomes {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    /// The current selected program this assignment describes.
    pub const fn selected(
        &self,
    ) -> &target_operations_to_selected_instructions::ValidatedSelectedInstructions {
        self.legality.selected()
    }
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.legality.register_environment()
    }
    /// The governing optimizer selections admitted with the retained stage.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.legality.selections()
    }
    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.legality.budget_per_pass()
    }
    pub const fn liveness(&self) -> &ValidatedLiveness {
        self.legality.liveness()
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        self.legality.ranges()
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        self.legality.legality()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.legality
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    assignment_context_accessors!();
    receipt_accessor_rows!(ref StagedOptimizedRegisterHomes {
        custody: StagedOptimizedRegisterHomeCustodyReceipt,
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedRegisterHomeCustodyReceipt {
    core: StagedOptimizedCustodyCore,
    allocator_availability: AllocatorAvailabilityIdentity,
    selected: SelectedInstructionPlanIdentity,
    liveness: LivenessIdentity,
    ranges: LiveRangeIdentity,
    legality: AllocationLegalityIdentity,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedRegisterHomeCustodyReceipt {
    target_operations_to_selected_instructions::staging_custody_core_accessors!();

    receipt_accessor_rows!(StagedOptimizedRegisterHomeCustodyReceipt {
        allocator_availability: AllocatorAvailabilityIdentity,
        selected: SelectedInstructionPlanIdentity,
        liveness: LivenessIdentity,
        ranges: LiveRangeIdentity,
        legality: AllocationLegalityIdentity,
        homes: RegisterHomeIdentity,
        post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
        function_count: usize,
        assignment_count: usize,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedRegisterHomeCustodyError {
    Assignment(RegisterHomeError),
    Revalidation(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedRegisterHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized register-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedRegisterHomeCustodyError {}

/// Physical homes after one exact fixed-view copy transformation and complete
/// reanalysis. This remains custody-only and cannot enter machine emission.
#[derive(Debug)]
pub(crate) struct StagedOptimizedRegisterHomesAfterFixedViewCopies {
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
    /// The retained reanalysis stage. Replay and custody validation inspect
    /// it; ordinary consumers read the current program and analyses directly.
    /// The transformed program this assignment describes.
    pub const fn selected(&self) -> &ValidatedFixedViewCopies {
        self.reanalysis.transformation_stage().copies()
    }
    pub const fn register_environment(
        &self,
    ) -> &register_environment::ValidatedTargetRegisterEnvironment {
        self.reanalysis.register_environment()
    }
    /// The governing optimizer selections admitted with the retained stage.
    pub fn selections(&self) -> &optimization_core::OptimizationSelections {
        self.reanalysis.selections()
    }
    /// The per-pass work budget admitted beside the same evidence.
    pub fn budget_per_pass(&self) -> optimization_core::OptimizationWorkBudget {
        self.reanalysis.budget_per_pass()
    }
    /// The reanalyzed facts over the transformed program.
    pub const fn liveness(&self) -> &ValidatedLiveness {
        self.reanalysis.liveness()
    }
    pub const fn ranges(&self) -> &ValidatedLiveRanges {
        self.reanalysis.ranges()
    }
    pub const fn legality(&self) -> &ValidatedAllocationLegality {
        self.reanalysis.legality()
    }
    /// The retained optimized-target proof input, kept as replay evidence;
    /// downstream custody checks compare the owner handle by identity.
    pub fn optimized_target_owner(
        &self,
    ) -> &std::sync::Arc<abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations>
    {
        self.reanalysis
            .transformation_stage()
            .source_legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target_owner()
    }
    assignment_context_accessors!();
    receipt_accessor_rows!(ref StagedOptimizedRegisterHomesAfterFixedViewCopies {
        custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    });
}

/// The post-copy route's receipt: the reanalysis custody it retained, over the
/// one post-home shape every staging route publishes.
pub type StagedOptimizedPostCopyRegisterHomeCustodyReceipt =
    PostHomeCustodyReceipt<StagedOptimizedSelectedReanalysisCustodyReceipt>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostCopyRegisterHomeCustodyError {
    UpstreamReanalysis(OptimizedSelectedReanalysisError),
    Assignment(RegisterHomeError),
    Revalidation(RegisterHomeError),
    Manifest(PostAllocationOptimizationManifestError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedPostCopyRegisterHomeCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-copy register-home staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostCopyRegisterHomeCustodyError {}
