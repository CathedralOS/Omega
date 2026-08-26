use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity, OptimizationWorkBudget,
    OptimizationWorkUsage, OptimizedAbstractPlanProjectionIdentity,
    PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    TerminalFixedViewCopyError, TerminalFixedViewCopyIdentity, TerminalFixedViewCopyPolicy,
    ValidatedTerminalFixedViewCopies, materialize_terminal_fixed_view_copies,
    validate_terminal_fixed_view_copies,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    validate_optimized_allocation_legality_custody,
};

/// Exact named fixed-view copy materialization over the complete source
/// legality chain. It mutates only its private selected-CFG realization and
/// grants no allocation, emission, or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedFixedViewCopies {
    source: StagedOptimizedAllocationLegality,
    copies: ValidatedTerminalFixedViewCopies,
    custody: StagedOptimizedFixedViewCopyCustodyReceipt,
}

impl StagedOptimizedFixedViewCopies {
    pub const fn source_legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.source
    }
    pub const fn copies(&self) -> &ValidatedTerminalFixedViewCopies {
        &self.copies
    }
    pub const fn custody(&self) -> StagedOptimizedFixedViewCopyCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedFixedViewCopyCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    allocator_availability: omega_regalloc::TerminalAllocatorAvailabilityIdentity,
    source_selected: TerminalSelectedInstructionPlanIdentity,
    source_liveness: omega_regalloc::TerminalLivenessIdentity,
    source_ranges: omega_regalloc::TerminalLiveRangeIdentity,
    source_legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    transformation: TerminalFixedViewCopyIdentity,
    transformed_selected: TerminalSelectedInstructionPlanIdentity,
    policy: TerminalFixedViewCopyPolicy,
    usage: OptimizationWorkUsage,
    function_count: usize,
    copy_count: usize,
}

impl StagedOptimizedFixedViewCopyCustodyReceipt {
    pub const fn terminal_psi(self) -> TerminalPsiIdentity {
        self.terminal_psi
    }
    pub const fn target(self) -> omega_target::NativeTarget {
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
    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(
        self,
    ) -> omega_regalloc::TerminalAllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn source_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn source_liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.source_liveness
    }
    pub const fn source_ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.source_ranges
    }
    pub const fn source_legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.source_legality
    }
    pub const fn transformation(self) -> TerminalFixedViewCopyIdentity {
        self.transformation
    }
    pub const fn transformed_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> TerminalFixedViewCopyPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn copy_count(self) -> usize {
        self.copy_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedFixedViewCopyCustodyError {
    UpstreamLegality(OptimizedAllocationLegalityCustodyError),
    Materialization(TerminalFixedViewCopyError),
    Revalidation(TerminalFixedViewCopyError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedFixedViewCopyCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized fixed-view copy staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedFixedViewCopyCustodyError {}

pub fn stage_optimized_fixed_view_copies(
    source: StagedOptimizedAllocationLegality,
    policy: TerminalFixedViewCopyPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedFixedViewCopies, OptimizedFixedViewCopyCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedFixedViewCopyCustodyError::UpstreamLegality)?;
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    let copies = materialize_terminal_fixed_view_copies(
        selected_stage.selected(),
        source.live_range_stage().ranges(),
        source.legality(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        policy,
        budget,
    )
    .map_err(OptimizedFixedViewCopyCustodyError::Materialization)?;
    let replayed =
        revalidate(&source, &copies).map_err(OptimizedFixedViewCopyCustodyError::Revalidation)?;
    if replayed.receipt() != copies.receipt() {
        return Err(OptimizedFixedViewCopyCustodyError::ReceiptMismatch);
    }
    let custody = custody_receipt(upstream, copies.receipt());
    Ok(StagedOptimizedFixedViewCopies {
        source,
        copies,
        custody,
    })
}

pub fn validate_optimized_fixed_view_copy_custody(
    source: &StagedOptimizedAllocationLegality,
    copies: &ValidatedTerminalFixedViewCopies,
) -> Result<StagedOptimizedFixedViewCopyCustodyReceipt, OptimizedFixedViewCopyCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedFixedViewCopyCustodyError::UpstreamLegality)?;
    let replayed =
        revalidate(source, copies).map_err(OptimizedFixedViewCopyCustodyError::Revalidation)?;
    if replayed.receipt() != copies.receipt() {
        return Err(OptimizedFixedViewCopyCustodyError::ReceiptMismatch);
    }
    Ok(custody_receipt(upstream, replayed.receipt()))
}

fn revalidate(
    source: &StagedOptimizedAllocationLegality,
    copies: &ValidatedTerminalFixedViewCopies,
) -> Result<ValidatedTerminalFixedViewCopies, TerminalFixedViewCopyError> {
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    validate_terminal_fixed_view_copies(
        selected_stage.selected(),
        source.live_range_stage().ranges(),
        source.legality(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        copies.plan().clone(),
    )
}

fn custody_receipt(
    upstream: crate::StagedOptimizedAllocationLegalityCustodyReceipt,
    copies: omega_regalloc::TerminalFixedViewCopyValidationReceipt,
) -> StagedOptimizedFixedViewCopyCustodyReceipt {
    StagedOptimizedFixedViewCopyCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        allocator_availability: upstream.allocator_availability(),
        source_selected: upstream.selected(),
        source_liveness: upstream.liveness(),
        source_ranges: upstream.ranges(),
        source_legality: upstream.legality(),
        transformation: copies.identity(),
        transformed_selected: copies.transformed_selected(),
        policy: copies.policy(),
        usage: copies.usage(),
        function_count: copies.function_count(),
        copy_count: copies.copy_count(),
    }
}
