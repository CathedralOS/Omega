use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, carriers::*, error::*, model::*, prelude::*,
};
use super::statistics::function_relative_statistics;

#[allow(clippy::too_many_arguments)]
pub(in crate::stages::realization::function_relative_realization) fn expected_fixed_frame_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let selected_lowering = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let allocation_recovery = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let function_relative =
        selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    let source = homes.custody();
    let post = homes.post_allocation_manifest().record();
    let selected = selected_stage.selected().receipt().identity();
    if !selected_lowering.is_empty()
        || !allocation_recovery.is_empty()
        || !post_allocation.is_empty()
        || !function_relative.is_empty()
        || post.selected_lowering_completion.is_some()
        || post.selected != selected
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != selected
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || layout.selected() != selected
        || layout.machine() != machine.machine().receipt().identity()
        || layout.pre_layout() != encoding.identity()
        || frame.receipt().post_allocation_machine() != machine.machine().receipt().identity()
        || protocol.receipt().frame_layout() != frame.receipt().identity()
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
        || exit_contract.contract().frame
            != (crate::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                layout: frame.receipt().identity(),
                protocol: protocol.receipt().identity(),
            })
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }

    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: selected_lowering.identity(),
        selected_lowering_completion: None,
        allocation_recovery_selections: allocation_recovery.identity(),
        post_allocation_machine_selections: post_allocation.identity(),
        function_relative_layout_selections: function_relative.identity(),
        pre_physical_manifest: source.manifest(),
        post_allocation_manifest: post.identity,
        selected,
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
        statistics: function_relative_statistics(layout)?,
        frame: FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
            layout: frame.receipt().identity(),
            protocol: protocol.receipt().identity(),
        },
        machine_emission: unavailable,
        section_placement: unavailable,
        symbols: unavailable,
        object_relocations: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionRelativeOptimizationRealizationManifest { record })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::stages::realization::function_relative_realization) fn fixed_frame_custody(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
        source: homes.custody(),
        machine: machine.machine().receipt().identity(),
        requirements: requirements.receipt().identity(),
        storage: storage.receipt().identity(),
        frame: frame.receipt().identity(),
        protocol: protocol.receipt().identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}
