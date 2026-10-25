//! Sequence the native transformations for both application and callable bodies.
//!
//! `lower_to_target` owns abstract optimization and target lowering. The caller
//! captures runtime layout companions from that result before `emit_target` runs
//! instruction selection, allocation, frame/encoding realization and emission.
//! That internal boundary preserves metadata error ordering; it is not a public
//! phase stop. Entry admission, callback discovery and image publication belong
//! to realization, not to the representation transformation sequence.

mod error;

use abstract_operations_to_target_operations::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
};
pub(crate) use error::{OptimizedVerifiedPhysicalPipelineError, TargetLoweringError};
use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use register_environment::baseline_target_register_environment;
use selected_instructions_to_register_homes::stage_register_allocation;
use selected_instructions_to_selected_instructions::optimize_selected_instructions;

/// Both application entries and separately emitted callables use these stages.
pub(crate) fn lower_to_target(
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: OptimizedTargetLoweringRequest<'_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedOptimizedTargetOperations, TargetLoweringError> {
    let selections = selections.selections();
    let optimized = abstract_operations_to_abstract_operations::optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        budget,
    )
    .map_err(TargetLoweringError::AbstractOptimization)?;
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized, request,
    )
    .map_err(TargetLoweringError::Target)?;
    Ok(target)
}

/// Emit a body after its caller has captured the target layout companions.
pub(crate) fn emit_target(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<native_artifact::FragmentObjectProjection, BodyEmissionError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    let selected =
        target_operations_to_selected_instructions::stage_optimized_instruction_selection(
            optimized_target,
            register_environment,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let selected = optimize_selected_instructions(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)?;
    let allocation = stage_register_allocation(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)?;
    let machine =
        register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan(
            &allocation.current(),
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let budget = allocation.current().budget_per_pass();
    let encoded = machine_emission::stage_fixed_frame_function_relative_realization(
        allocation, machine, budget,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
    let physical = StagedOptimizedFunctionFragmentEmissionSource::from(encoded);
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
