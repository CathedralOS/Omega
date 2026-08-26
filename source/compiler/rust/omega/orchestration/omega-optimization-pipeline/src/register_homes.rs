use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity,
};
use omega_regalloc::{
    TerminalRegisterHomeError, TerminalRegisterHomeIdentity, ValidatedTerminalRegisterHomes,
    assign_terminal_register_homes, validate_terminal_register_homes,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    validate_optimized_allocation_legality_custody,
};

/// Bounded opt-in physical-home staging. This lane admits only legality plans
/// with at least one shared legal candidate per VReg and no unresolved
/// fixed-view transition or spill requirement. It grants no machine-emission
/// or publication authority.
#[derive(Debug)]
pub struct StagedOptimizedRegisterHomes {
    legality: StagedOptimizedAllocationLegality,
    homes: ValidatedTerminalRegisterHomes,
    custody: StagedOptimizedRegisterHomeCustodyReceipt,
}

impl StagedOptimizedRegisterHomes {
    pub const fn legality_stage(&self) -> &StagedOptimizedAllocationLegality {
        &self.legality
    }
    pub const fn homes(&self) -> &ValidatedTerminalRegisterHomes {
        &self.homes
    }
    pub const fn custody(&self) -> StagedOptimizedRegisterHomeCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedRegisterHomeCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    liveness: omega_regalloc::TerminalLivenessIdentity,
    ranges: omega_regalloc::TerminalLiveRangeIdentity,
    legality: omega_regalloc::TerminalAllocationLegalityIdentity,
    homes: TerminalRegisterHomeIdentity,
    function_count: usize,
    assignment_count: usize,
}

impl StagedOptimizedRegisterHomeCustodyReceipt {
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
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn liveness(self) -> omega_regalloc::TerminalLivenessIdentity {
        self.liveness
    }
    pub const fn ranges(self) -> omega_regalloc::TerminalLiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> omega_regalloc::TerminalAllocationLegalityIdentity {
        self.legality
    }
    pub const fn homes(self) -> TerminalRegisterHomeIdentity {
        self.homes
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
    Assignment(TerminalRegisterHomeError),
    Revalidation(TerminalRegisterHomeError),
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

pub fn stage_optimized_register_homes(
    legality: StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        legality.live_range_stage(),
        legality.legality(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::UpstreamLegality)?;
    let ranges = legality.live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let homes = assign_terminal_register_homes(
        legality.legality(),
        ranges.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Assignment)?;
    let replayed = validate_terminal_register_homes(
        legality.legality(),
        ranges.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    let custody = custody_receipt(upstream, homes.receipt());
    Ok(StagedOptimizedRegisterHomes {
        legality,
        homes,
        custody,
    })
}

pub fn validate_optimized_register_home_custody(
    legality: &StagedOptimizedAllocationLegality,
    homes: &ValidatedTerminalRegisterHomes,
) -> Result<StagedOptimizedRegisterHomeCustodyReceipt, OptimizedRegisterHomeCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        legality.live_range_stage(),
        legality.legality(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::UpstreamLegality)?;
    let ranges = legality.live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed = validate_terminal_register_homes(
        legality.legality(),
        ranges.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    Ok(custody_receipt(upstream, replayed.receipt()))
}

fn custody_receipt(
    upstream: crate::StagedOptimizedAllocationLegalityCustodyReceipt,
    homes: omega_regalloc::TerminalRegisterHomeValidationReceipt,
) -> StagedOptimizedRegisterHomeCustodyReceipt {
    StagedOptimizedRegisterHomeCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: upstream.legality(),
        homes: homes.identity(),
        function_count: homes.function_count(),
        assignment_count: homes.assignment_count(),
    }
}
