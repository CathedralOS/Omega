use optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelectionIdentity,
};

use crate::function_realization::{function_relative_statistics, seal_function_relative_manifest};
use crate::{
    FunctionRelativeFrameDisposition, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::model::{OptimizedUnitFunctionRelativeRealizationError, UnitSavedReturnAddressFrame};
use super::source::validate_source;
use selected_instructions_to_register_homes::AllocationOutput;

pub(super) fn expected_manifest(
    current: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: Option<&UnitSavedReturnAddressFrame>,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let selections = current.selections();
    let source = validate_source(current)?;
    let post = current.post_allocation_manifest().record();
    let (expected_exit_frame, realization_frame) = match frame {
        Some(frame) => (
            machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: frame.layout().receipt().identity(),
                protocol: frame.protocol().receipt().identity(),
            },
            FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
                layout: frame.layout().receipt().identity(),
                protocol: frame.protocol().receipt().identity(),
            },
        ),
        None => (
            machine_code::WholeFunctionFrameDisposition::FramelessV1,
            FunctionRelativeFrameDisposition::Unavailable,
        ),
    };
    if post.selected_lowering_completion.is_some()
        || post.selected != source.selected()
        || post.target != layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != source.selected()
        || encoding.selected() != source.selected()
        || encoding.machine() != machine.machine().receipt().identity()
        || layout.selected() != source.selected()
        || layout.machine() != machine.machine().receipt().identity()
        || layout.pre_layout() != encoding.identity()
        || exit_contract.contract().selected != source.selected()
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
        || exit_contract.contract().frame != expected_exit_frame
        || frame.is_some_and(|frame| {
            frame.layout().receipt().post_allocation_machine()
                != machine.machine().receipt().identity()
                || frame.protocol().receipt().frame_layout() != frame.layout().receipt().identity()
        })
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: empty_phase_identity(selections, OptimizationExecutionPhase::SelectedLowering),
        selected_lowering_completion: None,
        allocation_recovery_selections: empty_phase_identity(selections, OptimizationExecutionPhase::AllocationRecovery),
        post_allocation_machine_selections: empty_phase_identity(selections, OptimizationExecutionPhase::PostAllocationMachine),
        function_relative_layout_selections: empty_phase_identity(selections, OptimizationExecutionPhase::FunctionRelativeLayout),
        pre_physical_manifest: source.manifest(),
        post_allocation_manifest: post.identity,
        selected: source.selected(),
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: None,
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: layout.target(),
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(layout)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Manifest)?,
        frame: realization_frame,
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    Ok(seal_function_relative_manifest(record))
}

fn empty_phase_identity(
    selections: &optimization_core::OptimizationSelections,
    phase: OptimizationExecutionPhase,
) -> OptimizationSelectionIdentity {
    selections.for_phase(phase).identity()
}
