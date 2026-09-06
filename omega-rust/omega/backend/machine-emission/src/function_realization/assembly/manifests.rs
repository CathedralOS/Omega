use super::super::prelude::*;
use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, error::*, model::*,
};
use super::allocation::{baseline_allocation_source, selected_lowering_source};
use super::rel8::{rel8_selected, validate_layout_optimization_manifest_roots};
use super::statistics::function_relative_statistics;
use selected_instructions_to_register_homes::AllocationOutput;

#[allow(clippy::too_many_arguments)]
pub(in crate::function_realization) fn expected_allocated_post_allocation_machine_manifest(
    allocation: &selected_instructions_to_register_homes::AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    baseline_encoding: &StagedOptimizedSelectedFormEncoding,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedMachineLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
    frame: Option<&super::super::FunctionRelativeFrame>,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selections = allocation.selections();
    let post = allocation.post_allocation_manifest().record();
    expected_post_allocation_machine_manifest(
        selections,
        selections
            .for_phase(OptimizationExecutionPhase::SelectedLowering)
            .identity(),
        post.selected_lowering_completion,
        post.pre_physical,
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
        frame,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::function_realization) fn expected_post_allocation_machine_manifest(
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
    layout: &ResolvedMachineLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
    frame: Option<&super::super::FunctionRelativeFrame>,
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
        || exit_contract.contract().frame
            != match frame {
                Some(frame) => machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
                    layout: frame.layout().receipt().identity(),
                    protocol: frame.protocol().receipt().identity(),
                },
                None => machine_code::WholeFunctionFrameDisposition::FramelessV1,
            }
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    let baseline_bytes = function_relative_statistics(baseline_layout.program())?.bytes;
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
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
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
        frame: match exit_contract.contract().frame {
            machine_code::WholeFunctionFrameDisposition::FramelessV1 => FunctionRelativeFrameDisposition::Unavailable,
            machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { layout, protocol } => FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 { layout, protocol },
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

fn exit_custody_matches(
    normalized: PostAllocationMachineOptimizationCustody,
    custody: machine_code::WholeFunctionExitLayoutCustody,
) -> bool {
    match custody {
        machine_code::WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization,
            artifact_identity,
        } => {
            optimization == normalized.optimization()
                && artifact_identity == normalized.artifact_identity()
        }
        machine_code::WholeFunctionExitLayoutCustody::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 {
            fusion,
        } => {
            normalized.optimization()
                == Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                && fusion.bytes() == normalized.artifact_identity()
        }
        machine_code::WholeFunctionExitLayoutCustody::Aarch64SelectShortestMovnSeededI64MaterializationV1 {
            materialization,
        } => {
            normalized.optimization()
                == Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                && materialization.bytes() == normalized.artifact_identity()
        }
        machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        | machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. } => {
            false
        }
    }
}

pub(in crate::function_realization) fn expected_manifest(
    allocation: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_optimization: &ResolvedLayoutOptimization,
    frame: Option<&super::super::UnitSavedReturnAddressFrame>,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let expected_frame = match frame {
        Some(frame) => machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
            layout: frame.layout().receipt().identity(),
            protocol: frame.protocol().receipt().identity(),
        },
        None => machine_code::WholeFunctionFrameDisposition::FramelessV1,
    };
    let source = selected_lowering_source(allocation)?;
    let completion = source.source();
    let selections = allocation.selections();
    let selected_lowering_selections = selections
        .for_phase(OptimizationExecutionPhase::SelectedLowering)
        .identity();
    let function_relative_layout_selections = selections
        .for_phase(OptimizationExecutionPhase::FunctionRelativeLayout)
        .identity();
    let post_allocation_machine_selections = selections
        .for_phase(OptimizationExecutionPhase::PostAllocationMachine)
        .identity();
    let post = allocation.post_allocation_manifest().record();
    if exit_contract.contract().frame != expected_frame
        || completion.selections() != selections.identity()
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
        || exit_contract.contract().resolved_layout != layout_optimization.layout().identity()
    {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    validate_layout_optimization_manifest_roots(baseline_layout, layout_optimization, selections)?;
    let final_layout = layout_optimization.layout();
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
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: final_layout.identity(),
        x86_branch_relaxation: layout_optimization.relaxation().map(StagedOptimizedX86BranchRelaxation::identity),
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics,
        frame: match frame {
            Some(frame) => FunctionRelativeFrameDisposition::CanonicalFixedFrameV1 {
                layout: frame.layout().receipt().identity(),
                protocol: frame.protocol().receipt().identity(),
            },
            None => FunctionRelativeFrameDisposition::Unavailable,
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

pub(in crate::function_realization) fn expected_direct_manifest(
    allocation: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_optimization: &ResolvedLayoutOptimization,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selections = allocation.selections();
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
    validate_layout_optimization_manifest_roots(baseline_layout, layout_optimization, selections)?;
    let layout = layout_optimization.layout();
    let relaxation = layout_optimization.relaxation().ok_or(
        FunctionRelativeOptimizationRealizationError::MissingFunctionRelativeLayoutOptimization,
    )?;
    let source = baseline_allocation_source(allocation)?;
    let selected = source.selected();
    let post = allocation.post_allocation_manifest().record();
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
        || relaxation.output() != layout.identity()
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine
            != machine.machine().receipt().identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
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
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: Some(relaxation.identity()),
        post_allocation_machine_optimization: None,
        whole_function_exit_contract: exit_contract.identity(),
        target: baseline_layout.target(),
        layout_policy: baseline_layout.policy(),
        scope: FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1,
        statistics: function_relative_statistics(layout)?,
        frame: FunctionRelativeFrameDisposition::Unavailable,
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
