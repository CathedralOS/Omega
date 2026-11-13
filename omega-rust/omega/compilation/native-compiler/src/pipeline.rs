/*
Sequence the native representation transformations for both application entries
and separately emitted callable bodies.

`lower_to_target` drives abstract optimization (`Omega 01`) and target lowering
(`Omega 02`). The caller captures runtime layout and installed-callback stack
companions from that result before `emit_target` runs instruction selection
(`Omega 03`), selected-instruction optimization (`Omega 04`), register
allocation (`Omega 05`), post-allocation machine planning (`Omega 06`),
layout-independent encoding (`Omega 07`), resolved layout (`Omega 08`), layout
optimization (`Omega 09`), and fragment object emission.
*/

use crate::observations::{NativePipelineObservation, NativePipelineStage};
use abstract_operations_to_target_operations::lower_optimized_to_target_operations;
use abstract_operations_to_target_operations::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
};
use machine_emission::{
    FunctionRelativeOptimizationRealizationError, StagedOptimizedFunctionFragmentEmissionSource,
};
use optimization_core::OptimizationExecutionPhase;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_environment::baseline_target_register_environment;
use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, stage_optimized_post_allocation_machine_plan,
};
use resolved_layout_to_resolved_layout::execute_resolved_layout_optimization;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;
use selected_instructions_to_register_homes::{SelectedProgramView, stage_register_allocation};
use selected_instructions_to_selected_instructions::{
    SelectedInstructionOptimizationError, SelectedOptimizationInput, optimize_selected_instructions,
};
use target_operations_to_selected_instructions::{
    OptimizedSelectionPipelineError, TargetSelectionInput, stage_optimized_instruction_selection,
};

pub(crate) enum TargetLoweringError {
    AbstractOptimization(abstract_operations_to_abstract_operations::AbstractOptimizationError),
    Target(abstract_operations_to_target_operations::LoweringError),
}

#[derive(Debug)]
pub(crate) enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(SelectedInstructionOptimizationError),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("common physical staging failed: ")?;
        match self {
            Self::SelectedOptimization(error) => {
                write!(formatter, "SelectedOptimization({error:?})")
            }
            Self::RegisterEnvironment(error) => write!(formatter, "RegisterEnvironment({error:?})"),
            Self::Selection(error) => write!(formatter, "Selection({error:?})"),
            Self::RegisterAllocation(error) => write!(formatter, "RegisterAllocation({error:?})"),
            Self::PostAllocationMachine(error) => {
                write!(formatter, "PostAllocationMachine({error:?})")
            }
            Self::FunctionRelativeRealization(error) => {
                write!(formatter, "FunctionRelativeRealization({error:?})")
            }
        }
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}

pub(crate) fn lower_to_target(
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: OptimizedTargetLoweringRequest<'_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    observation: &NativePipelineObservation,
) -> Result<ValidatedOptimizedTargetOperations, TargetLoweringError> {
    let selections = selections.selections();
    let optimized = {
        observation.enter(NativePipelineStage::AbstractOptimization);
        abstract_operations_to_abstract_operations::optimize_abstract_operations(
            input.into_optimization_input(),
            selections,
            &selections.project_psi(),
            budget,
        )
        .map_err(TargetLoweringError::AbstractOptimization)
    }?;
    observation.enter(NativePipelineStage::AbstractToTarget);
    let (program, lowering_facts) = optimized.into_lowering_parts();
    lower_optimized_to_target_operations(program, lowering_facts, request)
        .map_err(TargetLoweringError::Target)
}

pub(crate) fn emit_target(
    optimized_target: ValidatedOptimizedTargetOperations,
    observation: &NativePipelineObservation,
) -> Result<native_artifact::FragmentObjectProjection, BodyEmissionError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let target_input = std::sync::Arc::new(optimized_target);
    let selected = {
        observation.enter(NativePipelineStage::InstructionSelection);
        let optimized = target_input.optimized();
        let callable_roots = target_input
            .lowering_facts()
            .callable_invocation_roots(target_input.target_operations())
            .map_err(|reason| {
                BodyEmissionError::Object(vec![diagnostics::Diagnostic::error(reason)])
            })?;
        stage_optimized_instruction_selection(
            TargetSelectionInput {
                target: target_input.target_operations(),
                source: optimized.plan(),
                unit: optimized.unit(),
                origins: target_input.boundary_origins(),
                callable_roots: &callable_roots,
                installed_interrupts: target_input.lowering_facts().installed_interrupts(),
                installed_entry_execution: target_input.installed_entry_execution(),
                installed_stack_requirements: target_input.installed_stack_requirements(),
                optimization: optimized.identity_bundle().identity(),
                projection: optimized.projection(),
                manifest: optimized.pre_physical_manifest().identity,
                selections: optimized.selections().clone(),
                budget_per_pass: optimized.budget_per_pass(),
            },
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)
    }?;
    let selected = {
        observation.enter(NativePipelineStage::SelectedOptimization);
        optimize_selected_instructions(SelectedOptimizationInput {
            program: selected.selected_program(),
            custody: selected.custody().core(),
            register_environment: selected.register_environment().clone(),
            selections: selected.selections().clone(),
            budget_per_pass: selected.budget_per_pass(),
        })
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)
    }?;
    let allocation = {
        observation.enter(NativePipelineStage::RegisterAllocation);
        stage_register_allocation(selected.into_selected_output())
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)
    }?;
    let machine = {
        observation.enter(NativePipelineStage::PostAllocationMachine);
        let current = allocation.current();
        stage_optimized_post_allocation_machine_plan(
            allocation.facts(),
            current.register_environment(),
            current.selections(),
            current.budget_per_pass(),
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)
    }?;
    let current = allocation.current();
    let selected = current.selected();
    let registers = current.register_environment().physical();
    let frame = machine_emission::stage_function_frame(
        &current,
        &machine,
        machine_emission::frame_layout::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
        current.budget_per_pass(),
    )?;
    let encoding = {
        observation.enter(NativePipelineStage::SelectedFormEncoding);
        stage_optimized_layout_independent_selected_form_encoding(
            selected.current_program(),
            machine.machine().plan(),
            registers,
            Some(frame.layout().plan()),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Encoding)
    }?;
    let layout = {
        observation.enter(NativePipelineStage::ResolvedLayout);
        stage_optimized_resolved_selected_form_layout(
            selected.current_program(),
            machine.machine().plan(),
            registers,
            encoding.program(),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::Layout)
    }?;
    let optimized_layout = {
        observation.enter(NativePipelineStage::LayoutOptimization);
        execute_resolved_layout_optimization(
            registers,
            &layout.shared_program(),
            &current
                .selections()
                .project_phase(OptimizationExecutionPhase::FunctionRelativeLayout),
            current.budget_per_pass(),
        )
        .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)
    }?;
    let realization = machine_emission::assemble_fixed_frame_realization(
        allocation,
        target_input,
        machine,
        frame,
        encoding,
        layout,
        optimized_layout,
    )?;
    let physical = StagedOptimizedFunctionFragmentEmissionSource::from(realization);
    observation.enter(NativePipelineStage::FragmentEmission);
    native_artifact::emit_fragment_object(physical).map_err(BodyEmissionError::Object)
}

pub(crate) enum BodyEmissionError {
    Physical(OptimizedVerifiedPhysicalPipelineError),
    Object(Vec<diagnostics::Diagnostic>),
}

impl From<OptimizedVerifiedPhysicalPipelineError> for BodyEmissionError {
    fn from(error: OptimizedVerifiedPhysicalPipelineError) -> Self {
        Self::Physical(error)
    }
}

impl From<FunctionRelativeOptimizationRealizationError> for BodyEmissionError {
    fn from(error: FunctionRelativeOptimizationRealizationError) -> Self {
        Self::Physical(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization(error))
    }
}
