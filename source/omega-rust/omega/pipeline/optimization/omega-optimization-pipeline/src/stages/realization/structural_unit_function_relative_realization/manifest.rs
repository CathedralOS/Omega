use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, OptimizationExecutionPhase,
    OptimizationSelectionIdentity,
};
use omega_regalloc::ValidatedSelectedAnalysis;

use crate::stages::realization::function_relative_realization::{
    function_relative_statistics, seal_function_relative_manifest,
};
use crate::{
    FunctionRelativeOptimizationRealizationError, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedFunctionRelativeOptimizationRealizationManifest,
    ValidatedWholeFunctionExitContract,
};

use super::model::OptimizedStructuralUnitFunctionRelativeRealizationError;
use super::source::selected_stage;

pub(super) fn expected_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let selected_stage = selected_stage(homes);
    let optimized = selected_stage.optimized_target().optimized();
    let selections = optimized.selections();
    let source = homes.custody();
    let post = homes.post_allocation_manifest().record();
    let selected_plan = selected_stage.selected().selected_plan();
    let structural_function_count = u64::try_from(selected_plan.structural_unit_functions.len())
        .map_err(|_| {
            OptimizedStructuralUnitFunctionRelativeRealizationError::Manifest(
                FunctionRelativeOptimizationRealizationError::StatisticsOverflow,
            )
        })?;
    if post.selected_lowering_completion.is_some()
        || post.selected != source.selected()
        || post.statistics.functions != 0
        || post.statistics.structural_unit_functions != structural_function_count
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
    {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch);
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
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
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
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Manifest)?,
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

fn empty_phase_identity(
    selections: &omega_optimization_core::OptimizationSelections,
    phase: OptimizationExecutionPhase,
) -> OptimizationSelectionIdentity {
    selections.for_phase(phase).identity()
}
