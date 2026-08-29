use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelections,
};
use omega_regalloc::PostAllocationSelectedTransformation;

use crate::stages::realization::function_relative_realization::{
    function_relative_statistics, seal_function_relative_manifest,
};
use crate::{
    FunctionRelativeOptimizationRealizationManifest, FunctionRelativeOptimizationRealizationScope,
    FunctionRelativeOptimizationRealizationStage, FunctionRelativeOptimizationUnavailableData,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
    WholeFunctionExitLayoutCustody,
};

use super::model::OptimizedActiveResidentRematerializationFunctionRelativeRealizationError;
use super::source::artifacts;

pub(super) fn expected_manifest(
    source: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let artifacts = artifacts(source)?;
    let rematerialization = source.pre_layout().source();
    let selected_stage = rematerialization
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selections = selected_stage.optimized_target().optimized().selections();
    let empty = OptimizationSelections::default().identity();
    let source_custody = rematerialization.custody().source();
    let post = rematerialization.post_allocation_manifest().record();
    let rematerialization_identity = rematerialization.rematerialization().receipt().identity();
    let machine = artifacts.machine.machine().receipt();
    if post.pre_physical != source_custody.manifest()
        || post.selected_lowering_completion.is_some()
        || post.selected_transformations
            != [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization_identity,
                ),
            ]
        || post.selected
            != rematerialization
                .rematerialization()
                .receipt()
                .transformed_selected()
        || post.target != artifacts.layout.target()
        || machine.post_allocation_manifest() != post.identity
        || machine.selected() != post.selected
        || artifacts.encoding.selected() != post.selected
        || artifacts.encoding.machine() != machine.identity()
        || artifacts.layout.selected() != post.selected
        || artifacts.layout.machine() != machine.identity()
        || artifacts.layout.pre_layout() != artifacts.encoding.identity()
        || exit_contract.contract().selected != post.selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine.identity()
        || exit_contract.contract().pre_layout != artifacts.encoding.identity()
        || exit_contract.contract().resolved_layout != artifacts.layout.identity()
        || !matches!(
            exit_contract.contract().layout_custody,
            WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        )
    {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,
        );
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: empty,
        selected_lowering_completion: None,
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections: empty,
        function_relative_layout_selections: empty,
        pre_physical_manifest: source_custody.manifest(),
        post_allocation_manifest: post.identity,
        selected: post.selected,
        pre_allocation_machine_effects: artifacts
            .machine
            .effects()
            .effects()
            .receipt()
            .identity(),
        post_allocation_machine: machine.identity(),
        baseline_pre_layout: artifacts.encoding.identity(),
        pre_layout: artifacts.encoding.identity(),
        baseline_resolved_layout: artifacts.layout.identity(),
        resolved_layout: artifacts.layout.identity(),
        x86_branch_relaxation: None,
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: artifacts.layout.target(),
        layout_policy: artifacts.layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(artifacts.layout).map_err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Manifest,
        )?,
        frame: unavailable,
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
