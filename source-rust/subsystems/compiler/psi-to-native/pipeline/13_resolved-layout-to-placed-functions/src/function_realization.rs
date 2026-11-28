//! Check the exit contract against the layout, frame and allocation that placement consumes.

use std::sync::Arc;

use machine_code::OptimizedMachineLayout;

use optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use register_homes::AllocatedCalleeSavedRequirements;

use crate::exit_contract::{
    WholeFunctionExitContract, WholeFunctionExitContractError, check_exits,
    validate_exit_record_for_current_layout,
};
use frame_admission::frame_layout::{
    TargetFrameLayoutPolicy, ValidatedTargetFrameLayout,
    validate_non_authoritative_callee_save_storage, validate_target_frame_demands,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    ExitContract(WholeFunctionExitContractError),
    CalleeSaveStorage(frame_admission::frame_layout::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(frame_admission::frame_layout::TargetFrameLayoutError),
    FrameProtocol(frame_admission::frame_protocol::TargetFrameProtocolEncodingError),
    RootMismatch,
}

diagnostics::failed_display!(
    FunctionRelativeOptimizationRealizationError,
    "function-relative optimization realization failed"
);

impl std::error::Error for FunctionRelativeOptimizationRealizationError {}

fn validate_frame_geometry(
    program: &machine_code::ResolvedMachineProgram,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<ValidatedTargetFrameLayout, FunctionRelativeOptimizationRealizationError> {
    let environment = &program.context.environment;
    if program.frame.policy != TargetFrameLayoutPolicy::CanonicalOrdinaryCallFrameV1 {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let storage = validate_non_authoritative_callee_save_storage(
        requirements,
        environment,
        (*program.storage).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::CalleeSaveStorage)?;
    let frame = validate_target_frame_demands(
        Arc::clone(&program.frame_demands),
        requirements,
        &storage,
        environment,
        (*program.frame).clone(),
    )
    .map_err(FunctionRelativeOptimizationRealizationError::FrameLayout)?;
    // Audit encoding checked each byte against this frame. Rejoin that plan
    // to the protocol placed here and the independently reconstructed layout.
    let protocol = &program.protocol;
    if !frame.has_plan(&program.frame)
        || protocol.frame_layout != program.frame
        || protocol.target != environment.target()
        || protocol.register_environment != environment.identity()
        || protocol.physical_register_model != environment.physical().identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    Ok(frame)
}

/// The exit record must describe the same frame and instructions that placement will write.
pub(super) fn checked_exits(
    layout: &OptimizedMachineLayout,
) -> Result<WholeFunctionExitContract, FunctionRelativeOptimizationRealizationError> {
    let function_exits = crate::exit_contract::function_exits(layout);
    let layout_custody = function_exits.custody;
    let exit_contract = check_exits(&function_exits)
        .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let program = &layout.program;
    let machine = &program.context.machine;
    let encoding = &program.exit_requirements.encoding;
    let baseline_roots = &layout.baseline;
    let frame = &program.frame;
    let protocol = &program.protocol;
    let facts = &program.context.publication;
    let selections = facts.selections();
    validate_layout_optimization_roots(layout, selections)?;
    let layout = layout.layout();
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let post = facts.record();
    let selected = &machine.plan.selected;
    let post_allocation_run = program.context.phase;
    if post_allocation_run.complete_selection() != selections.identity()
        || post_allocation_run.phase_selections() != post_allocation.identity()
        || post.selected != *selected
        || machine.plan.post_allocation_manifest != *post
        || encoding.machine != machine.plan
        || baseline_roots.machine != machine.plan
        || baseline_roots.pre_layout != encoding.pre_layout
        || *layout.machine() != machine.plan
        || *layout.pre_layout() != encoding.pre_layout
        || frame.post_allocation_machine != machine.plan
        || protocol.frame_layout != *frame
        || exit_contract.post_allocation_machine != machine.plan
        || exit_contract.pre_layout != encoding.pre_layout
        || exit_contract.resolved_layout != program.layout
        || exit_contract.layout_custody != layout_custody
        || exit_contract.frame
            != (machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: Arc::clone(frame),
                protocol: Arc::clone(&program.protocol),
            })
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }

    Ok(exit_contract)
}

pub(super) fn verify_frame_requirements(
    layout: &OptimizedMachineLayout,
    exits: &WholeFunctionExitContract,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let program = &layout.program;
    let frame = validate_frame_geometry(program, requirements)?;
    validate_exit_record_for_current_layout(
        crate::exit_contract::function_exits(layout),
        Some((&frame, &program.protocol)),
        exits,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

/// Without relaxation the current layout is the baseline and keeps its roots.
/// With relaxation, the retained trace must be the one named, start from the
/// baseline roots and have produced the current layout.
fn validate_layout_optimization_roots(
    layout: &OptimizedMachineLayout,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections != phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let baseline = &layout.baseline;
    match (layout.relaxation, layout.trace.as_deref()) {
        (None, None) if layout.layout().roots() == *baseline => Ok(()),
        (Some(relaxation), Some(trace))
            if trace.identity() == relaxation
                && *trace.machine() == baseline.machine
                && *trace.pre_layout() == baseline.pre_layout
                && trace.layout == layout.program.layout =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}
