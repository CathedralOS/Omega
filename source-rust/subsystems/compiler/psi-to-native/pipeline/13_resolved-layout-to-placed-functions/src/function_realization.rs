//! Publish current resolved functions with their exit contract and manifest.
//! Explicit replay consumes retained originals and frame-admission checks.

use std::sync::Arc;

use machine_code::OptimizedMachineLayout;
use machine_code::ResolvedMachineLayout;
use machine_code::layout::X86BranchRelaxationRecord;
use optimization_core::{
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
};
use register_homes::AllocatedCalleeSavedRequirements;
use selected_instructions::SelectedInstructionPlan;

use crate::exit_contract::{
    StagedWholeFunctionExitContract, WholeFunctionExitContractError,
    stage_whole_function_exit_contract_for_layout, validate_exit_record_for_current_layout,
};
use frame_admission::frame_layout::{
    TargetFrameLayoutPolicy, ValidatedTargetFrameLayout,
    validate_non_authoritative_callee_save_storage, validate_target_frame_demands,
};

/// The roots later stages and audits read from a realized function: the
/// optimization selections it ran under, its selected program and its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRelativeOptimizationRealizationManifest {
    pub selections: OptimizationSelectionIdentity,
    pub allocation_recovery_selections: OptimizationSelectionIdentity,
    pub selected: Arc<SelectedInstructionPlan>,
    pub resolved_layout: Arc<ResolvedMachineLayout>,
}

/// Only assembly builds this manifest, after checking that every root of the
/// realized program agrees with the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionRelativeOptimizationRealizationManifest {
    record: FunctionRelativeOptimizationRealizationManifest,
}

impl ValidatedFunctionRelativeOptimizationRealizationManifest {
    pub const fn record(&self) -> &FunctionRelativeOptimizationRealizationManifest {
        &self.record
    }
}

/// The placed function retains its issued layout, exit contract and manifest.
#[derive(Debug)]
pub struct StagedFixedFrameFunctionRelativeRealization {
    layout_optimization: placement::SettledFunctionLayout,
    exit_contract: StagedWholeFunctionExitContract,
    manifest: ValidatedFunctionRelativeOptimizationRealizationManifest,
}

impl StagedFixedFrameFunctionRelativeRealization {
    pub fn relaxed_layout(&self) -> &placement::SettledFunctionLayout {
        &self.layout_optimization
    }

    pub fn program(&self) -> &machine_code::ResolvedMachineProgram {
        &self.layout_optimization.current().program
    }

    pub fn manifest(&self) -> &ValidatedFunctionRelativeOptimizationRealizationManifest {
        &self.manifest
    }
}

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

/// Publish the exit contract and manifest from the completed layout.
/// Concrete verification evidence accompanies the result; assembly does not
/// replay a producer or invoke compiler callbacks.
pub fn assemble_fixed_frame_realization(
    layout_optimization: placement::SettledFunctionLayout,
) -> Result<StagedFixedFrameFunctionRelativeRealization, FunctionRelativeOptimizationRealizationError>
{
    let exit_contract =
        stage_whole_function_exit_contract_for_layout(layout_optimization.function_exits())
            .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)?;
    let manifest = fixed_frame_manifest(
        layout_optimization.current(),
        layout_optimization.trace(),
        &exit_contract,
    )?;
    Ok(StagedFixedFrameFunctionRelativeRealization {
        layout_optimization,
        exit_contract,
        manifest,
    })
}

pub(crate) fn validate_fixed_frame_function_relative_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let requirements = &staged.program().context.preservation;
    check_published_realization(staged, requirements)
}

/// Check the frame geometry and the exit record against `requirements`. The
/// manifest needs no second check: its private fields were fixed at assembly
/// from the same program and exit contract this record still holds.
pub(crate) fn check_published_realization(
    staged: &StagedFixedFrameFunctionRelativeRealization,
    requirements: &AllocatedCalleeSavedRequirements,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let program = staged.program();
    let frame = validate_frame_geometry(program, requirements)?;
    validate_exit_record_for_current_layout(
        staged.layout_optimization.function_exits(),
        Some((&frame, &program.protocol)),
        &staged.exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

fn fixed_frame_manifest(
    layout_optimization: &OptimizedMachineLayout,
    relaxation_trace: Option<&X86BranchRelaxationRecord>,
    exit_contract: &StagedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = &layout_optimization.program;
    let machine = &program.context.machine;
    let encoding = &program.exit_requirements.encoding;
    let baseline_roots = &layout_optimization.baseline;
    let frame = &program.frame;
    let protocol = &program.protocol;
    let facts = &program.context.publication;
    let selections = facts.selections();
    validate_layout_optimization_manifest_roots(layout_optimization, relaxation_trace, selections)?;
    let layout = layout_optimization.layout();
    let layout_custody = match layout_optimization.relaxation {
        Some(relaxation) => {
            machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation,
            }
        }
        None => machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    };
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
        || exit_contract.contract().post_allocation_machine != machine.plan
        || exit_contract.contract().pre_layout != encoding.pre_layout
        || exit_contract.contract().resolved_layout != program.layout
        || exit_contract.contract().layout_custody != layout_custody
        || exit_contract.contract().frame
            != (machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: Arc::clone(frame),
                protocol: Arc::clone(&program.protocol),
            })
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }

    let record = FunctionRelativeOptimizationRealizationManifest {
        selections: selections.identity(),
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        selected: Arc::clone(selected),
        resolved_layout: Arc::clone(&program.layout),
    };
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

/// Without relaxation the current layout is the baseline and keeps its roots.
/// With relaxation, the retained trace must be the one named, start from the
/// baseline roots and have produced the current layout.
fn validate_layout_optimization_manifest_roots(
    layout: &OptimizedMachineLayout,
    relaxation_trace: Option<&X86BranchRelaxationRecord>,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections != phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let baseline = &layout.baseline;
    match (layout.relaxation, relaxation_trace) {
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
