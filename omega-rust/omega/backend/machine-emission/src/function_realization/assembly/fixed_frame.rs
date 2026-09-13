use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData, carriers::*, error::*, model::*, prelude::*,
};
use super::statistics::function_relative_statistics;
use selected_instructions_to_register_homes::{
    AllocationEvidence, AllocationOutput, PostAllocationSelectedTransformation,
    ValidatedSelectedAnalysis,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::function_realization) fn expected_fixed_frame_manifest(
    allocation: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_optimization: &ResolvedLayoutOptimization,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationError,
> {
    let selections = allocation.selections();
    super::rel8::validate_layout_optimization_manifest_roots(
        baseline_layout,
        layout_optimization,
        selections,
    )?;
    let layout = layout_optimization.layout();
    let relaxation = layout_optimization
        .relaxation()
        .map(|relaxation| relaxation.identity());
    let layout_custody = match relaxation {
        Some(relaxation) => {
            machine_code::WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
                relaxation,
            }
        }
        None => machine_code::WholeFunctionExitLayoutCustody::BaselineNearLayoutV1,
    };
    let selected_lowering = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let allocation_recovery = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    let post_allocation = selections.for_phase(OptimizationExecutionPhase::PostAllocationMachine);
    let function_relative =
        selections.for_phase(OptimizationExecutionPhase::FunctionRelativeLayout);

    let post = allocation.post_allocation_manifest().record();
    let selected = allocation.selected().selected_identity();
    let expected_transformations = match allocation.evidence() {
        AllocationEvidence::RegisterHomes(_) => Vec::new(),
        AllocationEvidence::FixedViewCopies(receipt) => {
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                receipt.source().source().transformation(),
            )]
        }
        AllocationEvidence::LiteralFolds(receipt) => receipt
            .source()
            .transformations()
            .iter()
            .copied()
            .map(PostAllocationSelectedTransformation::LiteralFold)
            .collect(),
        AllocationEvidence::SelectedLowering(receipt) => receipt
            .source()
            .iterations()
            .iter()
            .map(|iteration| PostAllocationSelectedTransformation::LiteralFold(iteration.fold()))
            .collect(),
        AllocationEvidence::ActiveResidentRematerialization(receipt) => vec![
            PostAllocationSelectedTransformation::PressureRematerialization(
                receipt.rematerialization(),
            ),
        ],
        AllocationEvidence::RuntimeSpill(identity) if *identity == post.identity => {
            // AllocationSource replay independently reconstructs every retained
            // spill rewrite and its cumulative manifest before granting this
            // identity. Consume that checked history, not an unbound roster.
            post.selected_transformations.clone()
        }
        _ => return Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    };
    // A complete named selected-lowering run binds its completion identity and
    // its exact phase projection. Any other evidence role must present an
    // empty selected-lowering phase, never a substituted suite.
    let (expected_lowering_selections, expected_lowering_completion) = match allocation.evidence() {
        AllocationEvidence::SelectedLowering(receipt) => (
            receipt.source().selected_lowering_selections(),
            Some(receipt.source().identity()),
        ),
        _ => (OptimizationSelections::default().identity(), None),
    };
    let pre_physical = allocation
        .target_input()
        .optimized()
        .pre_physical_manifest()
        .record()
        .identity;
    if selected_lowering.identity() != expected_lowering_selections
        || !post_allocation.is_empty()
        || post.pre_physical != pre_physical
        || post.selected_transformations != expected_transformations
        || post.selected_lowering_completion != expected_lowering_completion
        || post.selected != selected
        || machine.machine().receipt().post_allocation_manifest() != post.identity
        || machine.machine().receipt().selected() != selected
        || encoding.selected() != selected
        || encoding.machine() != machine.machine().receipt().identity()
        || baseline_layout.selected() != selected
        || baseline_layout.machine() != machine.machine().receipt().identity()
        || baseline_layout.pre_layout() != encoding.identity()
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
        || exit_contract.contract().layout_custody != layout_custody
        || exit_contract.contract().frame
            != (machine_code::WholeFunctionFrameDisposition::CanonicalFixedFrameV1 {
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
        selected_lowering_completion: expected_lowering_completion,
        allocation_recovery_selections: allocation_recovery.identity(),
        post_allocation_machine_selections: post_allocation.identity(),
        function_relative_layout_selections: function_relative.identity(),
        pre_physical_manifest: pre_physical,
        post_allocation_manifest: post.identity,
        selected,
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
        post_allocation_machine: machine.machine().receipt().identity(),
        baseline_pre_layout: encoding.identity(),
        pre_layout: encoding.identity(),
        baseline_resolved_layout: baseline_layout.identity(),
        resolved_layout: layout.identity(),
        x86_branch_relaxation: relaxation,
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
pub(in crate::function_realization) fn fixed_frame_custody(
    source: AllocationEvidence,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    frame: &ValidatedTargetFrameLayout,
    protocol: &ValidatedTargetFrameProtocolEncoding,
    exit_contract: &ValidatedWholeFunctionExitContract,
    manifest: &ValidatedFunctionRelativeOptimizationRealizationManifest,
) -> StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
    StagedFixedFrameFunctionRelativeRealizationCustodyReceipt {
        source,
        machine: machine.machine().receipt().identity(),
        requirements: requirements.receipt().identity(),
        storage: storage.receipt().identity(),
        frame: frame.receipt().identity(),
        protocol: protocol.receipt().identity(),
        exit_contract: exit_contract.identity(),
        realization: manifest.record.identity,
    }
}
