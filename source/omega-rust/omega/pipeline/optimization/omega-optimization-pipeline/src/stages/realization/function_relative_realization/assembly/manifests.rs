use super::super::prelude::*;
use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, error::*, model::*,
};
use super::rel8::{final_layout, rel8_selected, validate_relaxation_manifest_roots};
use super::statistics::function_relative_statistics;

#[allow(clippy::too_many_arguments)]
pub(in crate::stages::realization::function_relative_realization) fn expected_direct_post_allocation_machine_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
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
    let selections = selected_stage.optimized_target().optimized().selections();
    let post = homes.post_allocation_manifest().record();
    expected_post_allocation_machine_manifest(
        selections,
        OptimizationSelections::default().identity(),
        None,
        homes.custody().manifest(),
        post.identity,
        post.selected,
        post.target,
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::stages::realization::function_relative_realization) fn expected_selected_lowering_post_allocation_machine_manifest(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let run = homes.selected_lowering_run();
    let completion = run.custody();
    let selections = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let post = homes.post_allocation_manifest().record();
    expected_post_allocation_machine_manifest(
        selections,
        selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .identity(),
        Some(completion.identity()),
        completion.source().manifest(),
        post.identity,
        post.selected,
        post.target,
        machine,
        optimization,
        baseline_encoding,
        encoding,
        baseline_layout,
        layout,
        exit_contract,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expected_post_allocation_machine_manifest(
    selections: &OptimizationSelections,
    selected_lowering_selections: OptimizationSelectionIdentity,
    selected_lowering_completion: Option<SelectedLoweringOptimizationCompletionIdentity>,
    pre_physical_manifest: PrePhysicalOptimizationManifestIdentity,
    post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    selected: SelectedInstructionPlanIdentity,
    target: NativeTarget,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let normalized = optimization
        .custody()
        .ok_or(FunctionRelativeOptimizationRealizationError::OptimizationCustodyUnavailable)?;
    let selected_lowering_phase =
        selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let post_phase = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let layout_phase = selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if selected_lowering_selections != selected_lowering_phase.identity()
        || selected_lowering_completion.is_some() == selected_lowering_phase.is_empty()
        || post_phase.as_slice() != [normalized.optimization()]
        || !layout_phase.is_empty()
        || normalized.selections() != selections.identity()
        || normalized.post_allocation_machine_selections() != post_phase.identity()
        || normalized.source() != machine.machine().receipt().identity()
        || machine.machine().receipt().post_allocation_manifest() != post_allocation_manifest
        || machine.machine().receipt().selected() != selected
        || baseline_encoding.selected() != selected
        || baseline_encoding.machine() != machine.machine().receipt().identity()
        || baseline_encoding
            .post_allocation_machine_optimization()
            .is_some()
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || encoding.post_allocation_machine_optimization() != Some(normalized)
        || baseline_layout.pre_layout() != baseline_encoding.identity()
        || baseline_layout
            .post_allocation_machine_optimization()
            .is_some()
        || layout.pre_layout() != encoding.identity()
        || layout.post_allocation_machine_optimization() != Some(normalized)
        || baseline_layout.target() != target
        || layout.target() != target
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post_allocation_manifest
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
        || !exit_custody_matches(normalized, exit_contract.contract().layout_custody)
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let baseline_bytes = function_relative_statistics(baseline_layout)?.bytes;
    let final_statistics = function_relative_statistics(layout)?;
    let expected_shrink = normalized
        .expected_byte_savings()
        .ok_or(FunctionRelativeOptimizationRealizationError::StatisticsOverflow)?;
    if baseline_bytes.checked_sub(final_statistics.bytes) != Some(expected_shrink) {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion,
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections: post_phase.identity(),
        function_relative_layout_selections: layout_phase.identity(),
        pre_physical_manifest,
        post_allocation_manifest,
        selected,
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: baseline_encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: None,
        post_allocation_machine_optimization: Some(normalized),
        whole_function_exit_contract: exit_contract.identity(),
        target,
        layout_policy: layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: final_statistics,
        frame: unavailable,
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

fn exit_custody_matches(
    normalized: PostAllocationMachineOptimizationCustody,
    custody: crate::WholeFunctionExitLayoutCustody,
) -> bool {
    match custody {
        crate::WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization,
            artifact_identity,
        } => {
            optimization == normalized.optimization()
                && artifact_identity == normalized.artifact_identity()
        }
        crate::WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion,
        } => {
            normalized.optimization()
                == Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                && fusion.bytes() == normalized.artifact_identity()
        }
        crate::WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization,
        } => {
            normalized.optimization()
                == Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                && materialization.bytes() == normalized.artifact_identity()
        }
        crate::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        | crate::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. } => {
            false
        }
    }
}

pub(in crate::stages::realization::function_relative_realization) fn expected_manifest(
    homes: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let run = homes.selected_lowering_run();
    let completion = run.custody();
    let selections = run
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .selections();
    let selected_lowering_selections = selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let function_relative_layout_selections = selections
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .identity();
    let post_allocation_machine_selections = selections
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .identity();
    let post = homes.post_allocation_manifest().record();
    if completion.selections() != selections.identity()
        || completion.selected_lowering_selections() != selected_lowering_selections
        || post.selected_lowering_completion != Some(completion.identity())
        || post.selected != completion.final_selected()
        || post.target != baseline_layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != completion.final_selected()
        || encoding.selected() != completion.final_selected()
        || encoding.machine() != machine.machine().receipt().identity()
        || baseline_layout.selected() != completion.final_selected()
        || baseline_layout.machine() != machine.machine().receipt().identity()
        || baseline_layout.pre_layout() != encoding.identity()
        || exit_contract.contract().selected != completion.final_selected()
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout
            != final_layout(baseline_layout, relaxation).identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    validate_relaxation_manifest_roots(baseline_layout, relaxation, selections)?;
    let final_layout = final_layout(baseline_layout, relaxation);
    let statistics = function_relative_statistics(final_layout)?;
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion: Some(completion.identity()),
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections,
        function_relative_layout_selections,
        pre_physical_manifest: completion.source().manifest(),
        post_allocation_manifest: post.identity,
        selected: completion.final_selected(),
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: final_layout.identity(),
        x86_branch_relaxation: relaxation.map(StagedOptimizedX86BranchRelaxation::identity),
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics,
        frame: unavailable,
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

pub(in crate::stages::realization::function_relative_realization) fn expected_direct_manifest(
    homes: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: &StagedOptimizedX86BranchRelaxation,
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
    let selected_lowering_selections = selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let function_relative_layout_selections = selections
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .identity();
    let post_allocation_machine_selections = selections
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .identity();
    if !selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .is_empty()
        || !rel8_selected(selections, baseline_layout.target().architecture)?
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let source = homes.custody();
    let selected = source.selected();
    let post = homes.post_allocation_manifest().record();
    if post.selected_lowering_completion.is_some()
        || post.selected != selected
        || post.target != baseline_layout.target()
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != selected
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || baseline_layout.selected() != selected
        || baseline_layout.machine() != machine.machine().receipt().identity()
        || baseline_layout.pre_layout() != encoding.identity()
        || relaxation.source() != baseline_layout.identity()
        || relaxation.output() != relaxation.layout().identity()
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != relaxation.layout().identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let mut record = FunctionRelativeOptimizationRealizationManifest {
        identity: FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(
            b"pending",
        ),
        stage:
            FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections,
        selected_lowering_completion: None,
        allocation_recovery_selections: selections
            .for_phase(OptimizationExecutionPhase::AllocationRecovery)
            .identity(),
        post_allocation_machine_selections,
        function_relative_layout_selections,
        pre_physical_manifest: source.manifest(),
        post_allocation_manifest: post.identity,
        selected,
        pre_allocation_machine_effects: machine.effects().effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: relaxation.layout().identity(),
        x86_branch_relaxation: Some(relaxation.identity()),
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(relaxation.layout())?,
        frame: unavailable,
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
