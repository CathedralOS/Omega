//! Publish current resolved functions with their exit contract and manifest.
//! Explicit replay consumes retained originals and frame-admission checks.

use std::sync::Arc;

use machine_code::OptimizedMachineLayout;
use machine_code::ResolvedSelectedFormLayoutIdentity;
use optimization_core::{
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
};
use register_homes::AllocatedCalleeSavedRequirements;
use selected_instructions::SelectedInstructionPlanIdentity;

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
    pub selected: SelectedInstructionPlanIdentity,
    pub resolved_layout: ResolvedSelectedFormLayoutIdentity,
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
    // Audit encoding checked each byte against this frame. Rejoin that digest
    // to the protocol placed here and the independently reconstructed layout.
    let protocol = &program.protocol;
    if frame.receipt().identity() != program.frame_identity
        || protocol.frame_layout != program.frame_identity
        || protocol.target != environment.target()
        || protocol.register_environment != environment.identity()
        || protocol.physical_register_model != environment.physical().identity()
        || machine_code::target_frame_protocol_encoding_identity(protocol)
            != program.protocol_identity
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
    let manifest = fixed_frame_manifest(layout_optimization.current(), &exit_contract)?;
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
    exit_contract: &StagedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let program = &layout_optimization.program;
    let machine = &program.context.machine;
    let encoding = &program.exit_requirements.encoding;
    let baseline_layout = layout_optimization.baseline.identity;
    let baseline_roots = layout_optimization.baseline;
    let frame = &program.frame;
    let protocol = &program.protocol;
    let facts = &program.context.publication;
    let selections = facts.selections();
    validate_layout_optimization_manifest_roots(baseline_layout, layout_optimization, selections)?;
    let layout = layout_optimization.layout();
    let relaxation = layout_optimization
        .relaxation
        .map(|relaxation| relaxation.identity);
    let layout_custody = match relaxation {
        Some(relaxation) => {
            machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation,
            }
        }
        None => machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    };
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let post = facts.record();
    let selected = machine.selected;
    let post_allocation_run = program.context.phase;
    if post_allocation_run.complete_selection() != selections.identity()
        || post_allocation_run.phase_selections() != post_allocation.identity()
        || post_allocation_run.result() != machine.identity
        || post.selected != selected
        || machine.post_allocation_manifest != post.identity
        || machine.selected != selected
        || encoding.selected != selected
        || encoding.machine != machine.identity
        || baseline_roots.selected != selected
        || baseline_roots.machine != machine.identity
        || baseline_roots.pre_layout != encoding.identity
        || layout.selected() != selected
        || layout.machine() != machine.identity
        || layout.pre_layout() != encoding.identity
        || frame.post_allocation_machine != machine.identity
        || protocol.frame_layout != program.frame_identity
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine.identity
        || exit_contract.contract().pre_layout != encoding.identity
        || exit_contract.contract().resolved_layout != layout.identity()
        || exit_contract.contract().layout_custody != layout_custody
        || exit_contract.contract().frame
            != (machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: program.frame_identity,
                protocol: program.protocol_identity,
            })
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }

    let record = FunctionRelativeOptimizationRealizationManifest {
        selections: selections.identity(),
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        selected,
        resolved_layout: layout.identity(),
    };
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

fn validate_layout_optimization_manifest_roots(
    baseline_layout: ResolvedSelectedFormLayoutIdentity,
    layout: &OptimizedMachineLayout,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections != phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    match layout.relaxation {
        None if layout.layout().identity() == baseline_layout => Ok(()),
        Some(relaxation)
            if relaxation.source == baseline_layout
                && relaxation.output == layout.layout().identity() =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}
