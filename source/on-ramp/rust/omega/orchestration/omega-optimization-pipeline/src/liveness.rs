use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_regalloc::{
    TerminalLivenessError, TerminalLivenessIdentity, ValidatedTerminalLiveness,
    analyze_terminal_liveness, validate_terminal_liveness,
};
use omega_terminal_selected_instructions::TerminalSelectedInstructionPlanIdentity;
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    OptimizedSelectionCustodyError, StagedOptimizedSelectedInstructions,
    validate_optimized_selection_custody,
};

/// Opt-in liveness staging over the complete selected-instruction custody
/// carrier. This grants no interval, allocation, emission, or publication
/// authority.
#[derive(Debug)]
pub struct StagedOptimizedLiveness {
    selected: StagedOptimizedSelectedInstructions,
    liveness: ValidatedTerminalLiveness,
    custody: StagedOptimizedLivenessCustodyReceipt,
}

impl StagedOptimizedLiveness {
    pub const fn selected_stage(&self) -> &StagedOptimizedSelectedInstructions {
        &self.selected
    }

    pub const fn liveness(&self) -> &ValidatedTerminalLiveness {
        &self.liveness
    }

    pub const fn custody(&self) -> StagedOptimizedLivenessCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedLivenessCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    liveness: TerminalLivenessIdentity,
    function_count: usize,
    block_count: usize,
    virtual_register_count: usize,
    instruction_count: usize,
    successor_count: usize,
}

impl StagedOptimizedLivenessCustodyReceipt {
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

    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn liveness(self) -> TerminalLivenessIdentity {
        self.liveness
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn block_count(self) -> usize {
        self.block_count
    }

    pub const fn virtual_register_count(self) -> usize {
        self.virtual_register_count
    }

    pub const fn instruction_count(self) -> usize {
        self.instruction_count
    }

    pub const fn successor_count(self) -> usize {
        self.successor_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedLivenessCustodyError {
    UpstreamSelection(OptimizedSelectionCustodyError),
    Analysis(TerminalLivenessError),
    Revalidation(TerminalLivenessError),
    ReceiptMismatch,
}

impl std::fmt::Display for OptimizedLivenessCustodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "optimized liveness staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedLivenessCustodyError {}

pub fn stage_optimized_liveness(
    selected: StagedOptimizedSelectedInstructions,
) -> Result<StagedOptimizedLiveness, OptimizedLivenessCustodyError> {
    let upstream = validate_optimized_selection_custody(
        selected.optimized_target(),
        selected.register_environment(),
        selected.legalized(),
        selected.selected(),
    )
    .map_err(OptimizedLivenessCustodyError::UpstreamSelection)?;
    let liveness = analyze_terminal_liveness(selected.selected())
        .map_err(OptimizedLivenessCustodyError::Analysis)?;
    let replayed = validate_terminal_liveness(selected.selected(), liveness.plan().clone())
        .map_err(OptimizedLivenessCustodyError::Revalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(OptimizedLivenessCustodyError::ReceiptMismatch);
    }
    let validation = liveness.receipt();
    let custody = StagedOptimizedLivenessCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        selected: upstream.selected(),
        liveness: validation.identity(),
        function_count: validation.function_count(),
        block_count: validation.block_count(),
        virtual_register_count: validation.virtual_register_count(),
        instruction_count: validation.instruction_count(),
        successor_count: validation.successor_count(),
    };
    Ok(StagedOptimizedLiveness {
        selected,
        liveness,
        custody,
    })
}

pub fn validate_optimized_liveness_custody(
    selected: &StagedOptimizedSelectedInstructions,
    liveness: &ValidatedTerminalLiveness,
) -> Result<StagedOptimizedLivenessCustodyReceipt, OptimizedLivenessCustodyError> {
    let upstream = validate_optimized_selection_custody(
        selected.optimized_target(),
        selected.register_environment(),
        selected.legalized(),
        selected.selected(),
    )
    .map_err(OptimizedLivenessCustodyError::UpstreamSelection)?;
    let replayed = validate_terminal_liveness(selected.selected(), liveness.plan().clone())
        .map_err(OptimizedLivenessCustodyError::Revalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(OptimizedLivenessCustodyError::ReceiptMismatch);
    }
    let validation = replayed.receipt();
    Ok(StagedOptimizedLivenessCustodyReceipt {
        terminal_psi: upstream.terminal_psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        selected: upstream.selected(),
        liveness: validation.identity(),
        function_count: validation.function_count(),
        block_count: validation.block_count(),
        virtual_register_count: validation.virtual_register_count(),
        instruction_count: validation.instruction_count(),
        successor_count: validation.successor_count(),
    })
}
