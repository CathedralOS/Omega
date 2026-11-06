//! Sequence the native transformations for both application and callable bodies.
//!
//! `lower_to_target` owns abstract optimization and target lowering. The caller
//! captures runtime layout companions from that result before `emit_target` runs
//! instruction selection, allocation, frame/encoding realization and emission.
//! That internal boundary preserves metadata error ordering; it is not a public
//! phase stop. Entry admission, callback discovery and image publication belong
//! to realization, not to the representation transformation sequence.

mod error;

use crate::observations::{NativePipelineObservation, NativePipelineStage};
use abstract_operations_to_target_operations::lower_optimized_to_target_operations;
use abstract_operations_to_target_operations::{
    OptimizedTargetLoweringRequest, ValidatedOptimizedTargetOperations,
};
pub(crate) use error::{OptimizedVerifiedPhysicalPipelineError, TargetLoweringError};
use machine_emission::FunctionRelativeOptimizationRealizationError;
use machine_emission::StagedOptimizedFunctionFragmentEmissionSource;
use optimization_core::OptimizationExecutionPhase;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_environment::baseline_target_register_environment;
use register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan;
use resolved_layout_to_resolved_layout::execute_resolved_layout_optimization;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;
use selected_instructions_to_register_homes::{SelectedProgramView, stage_register_allocation};
use selected_instructions_to_selected_instructions::{
    SelectedOptimizationInput, optimize_selected_instructions,
};
use target_operations_to_selected_instructions::{
    TargetSelectionInput, stage_optimized_instruction_selection,
};

/// Both application entries and separately emitted callables use these stages.
pub(crate) fn lower_to_target(
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: OptimizedTargetLoweringRequest<'_>,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    budget: optimization_core::OptimizationWorkBudget,
    observation: &NativePipelineObservation,
) -> Result<ValidatedOptimizedTargetOperations, TargetLoweringError> {
    phase_profile::scope("omega.native.lower_to_target", || {
        let selections = selections.selections();
        let optimized = phase_profile::scope("omega.01", || {
            observation.enter(NativePipelineStage::AbstractOptimization);
            abstract_operations_to_abstract_operations::optimize_abstract_operations(
                input.into_optimization_input(),
                selections,
                &selections.project_psi(),
                budget,
            )
            .map_err(TargetLoweringError::AbstractOptimization)
        })?;
        phase_profile::scope("omega.02", || {
            observation.enter(NativePipelineStage::AbstractToTarget);
            let (program, lowering_facts) = optimized.into_lowering_parts();
            lower_optimized_to_target_operations(program, lowering_facts, request)
                .map_err(TargetLoweringError::Target)
        })
    })
}

/// Emit a body after its caller has captured the target layout companions.
pub(crate) fn emit_target(
    optimized_target: ValidatedOptimizedTargetOperations,
    observation: &NativePipelineObservation,
) -> Result<native_artifact::FragmentObjectProjection, BodyEmissionError> {
    let register_environment = baseline_target_register_environment(optimized_target.target())
        .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterEnvironment)?;
    // Backend emission reads the optimized-target product directly;
    // instruction selection borrows its plans and later stages never see it.
    let target_input = std::sync::Arc::new(optimized_target);
    let selected = phase_profile::scope("omega.03", || {
        observation.enter(NativePipelineStage::InstructionSelection);
        let optimized = target_input.optimized();
        stage_optimized_instruction_selection(
            TargetSelectionInput {
                target: target_input.target_operations(),
                source: optimized.plan(),
                unit: optimized.unit(),
                origins: target_input.boundary_origins(),
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
    })?;
    let selected = phase_profile::scope("omega.04", || {
        observation.enter(NativePipelineStage::SelectedOptimization);
        optimize_selected_instructions(SelectedOptimizationInput {
            program: selected.selected_program(),
            custody: selected.custody().core(),
            register_environment: selected.register_environment().clone(),
            selections: selected.selections().clone(),
            budget_per_pass: selected.budget_per_pass(),
        })
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedOptimization)
    })?;
    let allocation = phase_profile::scope("omega.05", || {
        observation.enter(NativePipelineStage::RegisterAllocation);
        stage_register_allocation(selected.into_selected_output())
            .map_err(OptimizedVerifiedPhysicalPipelineError::RegisterAllocation)
    })?;
    phase_profile::scope("omega.native.frame_realization", || {
        let machine = phase_profile::scope("omega.06", || {
            observation.enter(NativePipelineStage::PostAllocationMachine);
            let current = allocation.current();
            stage_optimized_post_allocation_machine_plan(
                allocation.facts(),
                current.register_environment(),
                current.selections(),
                current.budget_per_pass(),
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)
        })?;
        let current = allocation.current();
        let selected = current.selected();
        let registers = current.register_environment().physical();
        let frame = phase_profile::scope("omega.native.frame_realization.function_frame", || {
            machine_emission::stage_function_frame(
                &current,
                &machine,
                machine_emission::frame_layout::TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1,
                current.budget_per_pass(),
            )
        })?;
        let encoding = phase_profile::scope("omega.07", || {
            observation.enter(NativePipelineStage::SelectedFormEncoding);
            stage_optimized_layout_independent_selected_form_encoding(
                selected.current_program(),
                machine.machine().plan(),
                registers,
                Some(frame.layout().plan()),
            )
            .map_err(FunctionRelativeOptimizationRealizationError::Encoding)
        })?;
        let layout = phase_profile::scope("omega.08", || {
            observation.enter(NativePipelineStage::ResolvedLayout);
            stage_optimized_resolved_selected_form_layout(
                selected.current_program(),
                machine.machine().plan(),
                registers,
                encoding.program(),
            )
            .map_err(FunctionRelativeOptimizationRealizationError::Layout)
        })?;
        let optimized_layout = phase_profile::scope("omega.09", || {
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
        })?;
        let realization = phase_profile::scope(
            "omega.native.frame_realization.assemble_fixed_frame",
            || {
                machine_emission::assemble_fixed_frame_realization(
                    allocation,
                    target_input,
                    machine,
                    frame,
                    encoding,
                    layout,
                    optimized_layout,
                )
            },
        )?;
        let physical = StagedOptimizedFunctionFragmentEmissionSource::from(realization);
        phase_profile::scope(
            "omega.native.frame_realization.emit_fragment_object",
            || {
                observation.enter(NativePipelineStage::FragmentEmission);
                native_artifact::emit_fragment_object(physical).map_err(BodyEmissionError::Object)
            },
        )
    })
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
