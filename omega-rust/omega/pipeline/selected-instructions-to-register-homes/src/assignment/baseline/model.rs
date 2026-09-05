use crate::{
    PostAllocationOptimizationManifestError, RegisterHomeError, RegisterHomeIdentity,
    ValidatedPostAllocationOptimizationManifest, ValidatedRegisterHomes,
};
use optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use selected_instructions::SelectedInstructionPlanIdentity;
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use terminal_psi::TerminalPsiIdentity;

use crate::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};
use crate::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt,
};

/// Bounded opt-in physical-home staging. This lane admits only legality plans
/// with at least one shared legal candidate per VReg and no unresolved
/// fixed-view transition or spill requirement. It grants no machine-emission
/// or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomes {
    pub(super) legality: StagedOptimizedAllocationLegality,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: StagedOptimizedRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomes {
    pub const fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.legality
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
    pub(super) psi: TerminalPsiIdentity,
    pub(super) target: target::NativeTarget,
    pub(super) entry: MachineId,
    pub(super) optimization: OptimizationIdentityBundleIdentity,
    pub(super) projection: OptimizedAbstractPlanProjectionIdentity,
    pub(super) manifest: PrePhysicalOptimizationManifestIdentity,
    pub(super) optimization_unit: OptimizationUnitIdentity,
    pub(super) fuel_schedule: FuelScheduleIdentity,
    pub(super) register_environment: register_model::TargetRegisterEnvironmentIdentity,
    pub(super) allocator_availability: crate::AllocatorAvailabilityIdentity,
    pub(super) selected: SelectedInstructionPlanIdentity,
    pub(super) liveness: crate::LivenessIdentity,
    pub(super) ranges: crate::LiveRangeIdentity,
    pub(super) legality: crate::AllocationLegalityIdentity,
    pub(super) homes: RegisterHomeIdentity,
    pub(super) post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub(super) function_count: usize,
    pub(super) structural_unit_function_count: usize,
    pub(super) assignment_count: usize,
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
    pub const fn allocator_availability(self) -> crate::AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn selected(self) -> SelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> crate::LivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> crate::LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> crate::AllocationLegalityIdentity {
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
    pub const fn structural_unit_function_count(self) -> usize {
        self.structural_unit_function_count
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
    pub(super) reanalysis: StagedOptimizedSelectedReanalysis,
    pub(super) homes: ValidatedRegisterHomes,
    pub(super) manifest: ValidatedPostAllocationOptimizationManifest,
    pub(super) custody: StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomesAfterFixedViewCopies {
    pub const fn reanalysis_stage(&self) -> &StagedOptimizedSelectedReanalysis {
        &self.reanalysis
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
    pub(super) source: StagedOptimizedSelectedReanalysisCustodyReceipt,
    pub(super) homes: RegisterHomeIdentity,
    pub(super) post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub(super) function_count: usize,
    pub(super) assignment_count: usize,
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
