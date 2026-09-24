//! Optimizer module role: executable entrance. Replayed register-home assignment.
//!
//! Baseline legality and post-copy reanalysis are distinct source families.
//! This entrance grants custody only after each constructed home/manifest pair
//! independently replays through its matching source-family validator.

mod construction;
mod custody;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
mod validation;

#[cfg(any(test, feature = "test-support"))]
pub use test_support::{
    OptimizedPostCopyRegisterHomeCustodyFieldForTest, OptimizedRegisterHomeCustodyFieldForTest,
};
pub use validation::{
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};

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
    OptimizedAllocationLegalityCustodyError, OptimizedSelectedReanalysisError,
    StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, ValidatedAllocationLegality,
    ValidatedFixedViewCopies, ValidatedLiveRanges, ValidatedLiveness,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::TerminalPsiIdentity;

pub fn stage_optimized_register_homes(
    legality: StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let staged = construction::construct_optimized_register_homes(legality)?;
    let custody = validate_optimized_register_home_custody(
        staged.legality_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}

pub(crate) fn stage_register_homes_with_assignment(
    legality: StagedOptimizedAllocationLegality,
    homes: crate::ValidatedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let staged = construction::construct_with_assignment(legality, homes)?;
    let custody = validate_optimized_register_home_custody(
        staged.legality_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
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
    let custody = validate_optimized_register_home_after_fixed_view_copy_custody(
        staged.reanalysis_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedPostCopyRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}

pub fn stage_optimized_register_homes_after_fixed_view_copies(
    reanalysis: StagedOptimizedSelectedReanalysis,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let staged =
        construction::construct_optimized_register_homes_after_fixed_view_copies(reanalysis)?;
    let custody = validate_optimized_register_home_after_fixed_view_copy_custody(
        staged.reanalysis_stage(),
        staged.homes(),
        staged.post_allocation_manifest(),
    )?;
    if custody != staged.custody() {
        return Err(OptimizedPostCopyRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(staged)
}

/// Bounded opt-in physical-home staging. This lane admits only legality plans
/// with at least one shared legal candidate per VReg and no unresolved
/// fixed-view transition or spill requirement. It grants no machine-emission
/// or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomes {
    legality: StagedOptimizedAllocationLegality,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomes {
    /// The retained producer stage. Replay and custody validation inspect it;
    /// ordinary consumers read the current program and analyses directly.
    pub const fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.legality
    }
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
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedRegisterHomeCustodyReceipt {
    psi: TerminalPsiIdentity,
    target: target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: register_model::TargetRegisterEnvironmentIdentity,
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
    pub const fn psi(self) -> TerminalPsiIdentity {
        self.psi
    }
    pub const fn target(self) -> target::NativeTarget {
        self.target
    }
    pub const fn entry(self) -> MachineId {
        self.entry
    }
    pub const fn optimization(self) -> OptimizationIdentityBundleIdentity {
        self.optimization
    }
    pub const fn projection(self) -> OptimizedAbstractPlanProjectionIdentity {
        self.projection
    }
    pub const fn manifest(self) -> PrePhysicalOptimizationManifestIdentity {
        self.manifest
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn register_environment(self) -> register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedRegisterHomeCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
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
pub struct StagedOptimizedRegisterHomesAfterFixedViewCopies {
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: ValidatedRegisterHomes,
    manifest: ValidatedPostAllocationOptimizationManifest,
    custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
    /// The retained reanalysis stage. Replay and custody validation inspect
    /// it; ordinary consumers read the current program and analyses directly.
    pub const fn reanalysis_stage(&self) -> &StagedOptimizedSelectedReanalysis {
        &self.reanalysis
    }
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
    pub const fn homes(&self) -> &ValidatedRegisterHomes {
        &self.homes
    }
    pub const fn post_allocation_manifest(&self) -> &ValidatedPostAllocationOptimizationManifest {
        &self.manifest
    }
    pub const fn custody(&self) -> StagedOptimizedPostCopyRegisterHomeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedPostCopyRegisterHomeCustodyReceipt {
    source: StagedOptimizedSelectedReanalysisCustodyReceipt,
    homes: RegisterHomeIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedPostCopyRegisterHomeCustodyReceipt {
    pub const fn source(self) -> StagedOptimizedSelectedReanalysisCustodyReceipt {
        self.source
    }
    pub const fn homes(self) -> RegisterHomeIdentity {
        self.homes
    }
    pub const fn post_allocation_manifest(self) -> PostAllocationOptimizationManifestIdentity {
        self.post_allocation_manifest
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn assignment_count(self) -> usize {
        self.assignment_count
    }
}

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
