use omega_lowering_optimizer::ValidatedOptimizedTargetOperations;
use omega_optimization_core::{
    OptimizationIdentityBundleIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizedAbstractPlanProjectionIdentity, PrePhysicalOptimizationManifestIdentity,
};
use omega_terminal_legalized_operations::TerminalLegalizedLeafValue;
use omega_terminal_selected_instructions::{
    TerminalSelectedFixedInputConstraint, TerminalSelectedInstructionPlanIdentity,
    TerminalSelectedSelectionConstraints,
};
use omega_terminal_target_operations::MachineRegister;
use omega_terminal_target_operations_to_selected_instructions::{
    SelectedInstructionError, TerminalLegalizationError, ValidatedTerminalLegalizedOperations,
    ValidatedTerminalSelectedInstructions, legalize_terminal_target_operations,
    select_terminal_instructions, validate_terminal_legalized_operations,
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
    legalized: ValidatedTerminalLegalizedOperations,
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

    pub const fn legalized(&self) -> &ValidatedTerminalLegalizedOperations {
        &self.legalized
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
    manifest: PrePhysicalOptimizationManifestIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
    register_environment: omega_register_model::TargetRegisterEnvironmentIdentity,
    legalized: omega_terminal_legalized_operations::TerminalLegalizedOperationPlanIdentity,
    legalization_validator: OptimizationValidatorIdentity,
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

    pub const fn legalized(
        self,
    ) -> omega_terminal_legalized_operations::TerminalLegalizedOperationPlanIdentity {
        self.legalized
    }

    pub const fn legalization_validator(self) -> OptimizationValidatorIdentity {
        self.legalization_validator
    }

    pub const fn register_environment(
        self,
    ) -> omega_register_model::TargetRegisterEnvironmentIdentity {
        self.register_environment
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
    LegalizedPlanRevalidationFailed,
    LegalizedReceiptMismatch,
    SelectedPlanRevalidationFailed,
    SelectedReceiptMismatch,
}

#[derive(Debug)]
pub enum OptimizedSelectionPipelineError {
    RegisterEnvironment(TargetRegisterEnvironmentValidationError),
    Legalization(TerminalLegalizationError),
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
    let legalized = legalize_terminal_target_operations(
        optimized_target.target_operations(),
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
    )
    .map_err(OptimizedSelectionPipelineError::Legalization)?;
    let selection_constraints = selection_constraints(&legalized, &register_environment);
    let selected = select_terminal_instructions(
        &legalized,
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
    )
    .map_err(OptimizedSelectionPipelineError::Selection)?;
    let custody = validate_optimized_selection_custody(
        &optimized_target,
        &register_environment,
        &legalized,
        &selected,
    )
    .map_err(OptimizedSelectionPipelineError::Custody)?;
    Ok(StagedOptimizedSelectedInstructions {
        optimized_target,
        register_environment,
        legalized,
        selected,
        custody,
    })
}

pub fn validate_optimized_selection_custody(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    legalized: &ValidatedTerminalLegalizedOperations,
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
    let relegalized = validate_terminal_legalized_operations(
        target,
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
        legalized.plan().clone(),
    )
    .map_err(|_| OptimizedSelectionCustodyError::LegalizedPlanRevalidationFailed)?;
    if relegalized.receipt() != legalized.receipt() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
    }
    let selection_constraints = selection_constraints(legalized, register_environment);
    let revalidated = validate_terminal_selected_instructions(
        legalized,
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
    if selected.receipt().legalized() != legalized.receipt().identity() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
    }
    if selected.receipt().legalization_validator() != legalized.receipt().validator() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
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
        manifest: optimized_target
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        register_environment: register_environment.identity(),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        selected: selected.receipt().identity(),
        function_count: plan.functions.len(),
    })
}

pub(crate) fn selection_constraints(
    legalized: &ValidatedTerminalLegalizedOperations,
    environment: &ValidatedTargetRegisterEnvironment,
) -> TerminalSelectedSelectionConstraints {
    let mut fixed_inputs = Vec::new();
    for function in &legalized.plan().functions {
        push_fixed_input(
            &mut fixed_inputs,
            environment,
            function.machine,
            function.condition_source,
            function.condition_parameter_index,
            function.condition_register,
        );
        for arm in [&function.when_true, &function.when_false] {
            let TerminalLegalizedLeafValue::EntryParameter {
                parameter_index,
                register,
                ..
            } = &arm.value
            else {
                continue;
            };
            push_fixed_input(
                &mut fixed_inputs,
                environment,
                function.machine,
                arm.source_value,
                *parameter_index,
                *register,
            );
        }
    }
    TerminalSelectedSelectionConstraints {
        keys: environment.selected_keys(),
        fixed_inputs,
    }
}

fn push_fixed_input(
    inputs: &mut Vec<TerminalSelectedFixedInputConstraint>,
    environment: &ValidatedTargetRegisterEnvironment,
    machine: MachineId,
    source_value: psi_core::ValueId,
    parameter_index: usize,
    register: MachineRegister,
) {
    if inputs.iter().any(|input| {
        input.machine == machine
            && input.source_value == source_value
            && input.parameter_index == parameter_index
            && input.register == register
    }) {
        return;
    }
    let Some(fixed_view) = environment.fixed_register_view(register) else {
        return;
    };
    inputs.push(TerminalSelectedFixedInputConstraint {
        machine,
        source_value,
        parameter_index,
        register,
        fixed_view,
    });
}
