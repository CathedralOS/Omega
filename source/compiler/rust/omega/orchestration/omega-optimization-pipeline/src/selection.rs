use omega_lowering_optimizer::ValidatedOptimizedTargetOperations;
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity,
    OptimizedAbstractPlanProjectionIdentity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedFixedInputConstraint, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedSelectionConstraints,
};
use omega_terminal_target_operations::{TerminalScalarParameterLocation, TerminalTargetOperation};
use omega_terminal_target_operations_to_selected_instructions::{
    SelectedInstructionError, ValidatedTerminalSelectedInstructions, select_terminal_instructions,
    validate_terminal_selected_instructions,
};
use psi_core::{FuelScheduleIdentity, MachineId};
use psi_terminal::TerminalPsiIdentity;

use crate::{
    TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment,
    baseline_target_register_environment,
};

/// Opt-in selected-instruction staging with complete optimized lowering and
/// target-register custody. This grants no liveness, allocation, emission, or
/// publication authority.
#[derive(Debug)]
pub struct StagedOptimizedSelectedInstructions {
    optimized_target: ValidatedOptimizedTargetOperations,
    register_environment: ValidatedTargetRegisterEnvironment,
    selected: ValidatedTerminalSelectedInstructions,
    custody: StagedOptimizedSelectionCustodyReceipt,
}

impl StagedOptimizedSelectedInstructions {
    pub const fn optimized_target(&self) -> &ValidatedOptimizedTargetOperations {
        &self.optimized_target
    }

    pub const fn register_environment(&self) -> &ValidatedTargetRegisterEnvironment {
        &self.register_environment
    }

    pub const fn selected(&self) -> &ValidatedTerminalSelectedInstructions {
        &self.selected
    }

    pub const fn custody(&self) -> StagedOptimizedSelectionCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedSelectionCustodyReceipt {
    terminal_psi: TerminalPsiIdentity,
    target: omega_target::NativeTarget,
    entry: MachineId,
    optimization: OptimizationIdentityBundleIdentity,
    projection: OptimizedAbstractPlanProjectionIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    selected: TerminalSelectedInstructionPlanIdentity,
    function_count: usize,
}

impl StagedOptimizedSelectionCustodyReceipt {
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

    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }

    pub const fn function_count(self) -> usize {
        self.function_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedSelectionCustodyError {
    RootMismatch,
    RegisterEnvironmentTargetMismatch,
    UnitIdentityMismatch,
    FuelScheduleMismatch,
    FunctionRosterMismatch,
    SelectedPlanRevalidationFailed,
    SelectedReceiptMismatch,
}

#[derive(Debug)]
pub enum OptimizedSelectionPipelineError {
    RegisterEnvironment(TargetRegisterEnvironmentValidationError),
    Selection(SelectedInstructionError),
    Custody(OptimizedSelectionCustodyError),
}

impl std::fmt::Display for OptimizedSelectionPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized instruction selection failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedSelectionPipelineError {}

pub fn stage_optimized_instruction_selection(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedSelectedInstructions, OptimizedSelectionPipelineError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedSelectionPipelineError::RegisterEnvironment)?;
    let selection_constraints = selection_constraints(&optimized_target, &register_environment);
    let selected = select_terminal_instructions(
        optimized_target.target_operations(),
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
    )
    .map_err(OptimizedSelectionPipelineError::Selection)?;
    let custody =
        validate_optimized_selection_custody(&optimized_target, &register_environment, &selected)
            .map_err(OptimizedSelectionPipelineError::Custody)?;
    Ok(StagedOptimizedSelectedInstructions {
        optimized_target,
        register_environment,
        selected,
        custody,
    })
}

pub fn validate_optimized_selection_custody(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    selected: &ValidatedTerminalSelectedInstructions,
) -> Result<StagedOptimizedSelectionCustodyReceipt, OptimizedSelectionCustodyError> {
    let target = optimized_target.target_operations();
    let plan = selected.plan();
    if target.terminal_psi != plan.terminal_psi
        || target.target != plan.target
        || target.entry != plan.entry
        || optimized_target.target() != target.target
    {
        return Err(OptimizedSelectionCustodyError::RootMismatch);
    }
    if register_environment.target() != target.target {
        return Err(OptimizedSelectionCustodyError::RegisterEnvironmentTargetMismatch);
    }
    let selection_constraints = selection_constraints(optimized_target, register_environment);
    let revalidated = validate_terminal_selected_instructions(
        target,
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
        selected.plan().clone(),
    )
    .map_err(|_| OptimizedSelectionCustodyError::SelectedPlanRevalidationFailed)?;
    if revalidated.receipt() != selected.receipt() {
        return Err(OptimizedSelectionCustodyError::SelectedReceiptMismatch);
    }
    let unit = optimized_target.optimized().unit();
    if selected.receipt().optimization_unit() != unit.identity {
        return Err(OptimizedSelectionCustodyError::UnitIdentityMismatch);
    }
    if selected.receipt().fuel_schedule() != unit.fuel_schedule
        || plan.fuel_schedule != unit.fuel_schedule
    {
        return Err(OptimizedSelectionCustodyError::FuelScheduleMismatch);
    }
    if target.functions.len() != plan.functions.len()
        || target
            .functions
            .iter()
            .zip(&plan.functions)
            .any(|(target, selected)| {
                target.machine != selected.machine
                    || target.attachment != selected.attachment
                    || target.provenance != selected.provenance
            })
    {
        return Err(OptimizedSelectionCustodyError::FunctionRosterMismatch);
    }
    Ok(StagedOptimizedSelectionCustodyReceipt {
        terminal_psi: target.terminal_psi,
        target: target.target,
        entry: target.entry,
        optimization: optimized_target.optimized().identity_bundle().identity(),
        projection: optimized_target.optimized().validation().identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        selected: selected.receipt().identity(),
        function_count: plan.functions.len(),
    })
}

pub(crate) fn selection_constraints(
    optimized_target: &ValidatedOptimizedTargetOperations,
    environment: &ValidatedTargetRegisterEnvironment,
) -> TerminalSelectedSelectionConstraints {
    let fixed_inputs = optimized_target
        .target_operations()
        .functions
        .iter()
        .filter_map(|function| {
            let TerminalTargetOperation::ReturnIntegerConditionalControl {
                condition_source,
                condition_parameter_index,
                condition_location: TerminalScalarParameterLocation::Register(register),
                ..
            } = function.operation
            else {
                return None;
            };
            Some(TerminalSelectedFixedInputConstraint {
                machine: function.machine,
                source_value: condition_source,
                parameter_index: condition_parameter_index,
                register,
                fixed_view: environment.fixed_register_view(register)?,
            })
        })
        .collect();
    TerminalSelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs,
    }
}
