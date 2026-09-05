use omega_optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use omega_selected_instructions_to_register_homes::PostAllocationSelectedTransformation;

use crate::function_realization::{function_relative_statistics, seal_function_relative_manifest};
use crate::{
    FunctionRelativeFrameDisposition, FunctionRelativeOptimizationRealizationManifest,
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
    ValidatedFunctionRelativeOptimizationRealizationManifest, ValidatedWholeFunctionExitContract,
};
use omega_machine_code::WholeFunctionExitLayoutCustody;
use omega_post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use omega_selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::model::AllocationRecoveryFunctionRelativeRealizationError;
use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use omega_selected_instructions_to_register_homes::{AllocationEvidence, AllocationOutput};

pub(super) fn expected_manifest(
    source: &AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<
    ValidatedFunctionRelativeOptimizationRealizationManifest,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    let selections = source.target_input().optimized().selections();
    let post = source.post_allocation_manifest().record();
    let selected = source.selected().selected_identity();
    let machine_receipt = machine.machine().receipt();
    let expected_transformations = match source.evidence() {
        AllocationEvidence::FixedViewCopies(receipt) => {
            vec![PostAllocationSelectedTransformation::FixedViewCopy(
                receipt.source().source().transformation(),
            )]
        }
        AllocationEvidence::ActiveResidentRematerialization(receipt) => vec![
            PostAllocationSelectedTransformation::PressureRematerialization(
                receipt.rematerialization(),
            ),
        ],
        _ => return Err(AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections),
    };
    if post.pre_physical
        != source
            .target_input()
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity
        || post.selected_lowering_completion.is_some()
        || post.selected_transformations != expected_transformations
        || post.selected != selected
        || machine_receipt.post_allocation_manifest() != post.identity
        || machine_receipt.selected() != selected
        || encoding.selected() != selected
        || encoding.machine() != machine_receipt.identity()
        || layout.selected() != selected
        || layout.machine() != machine_receipt.identity()
        || layout.pre_layout() != encoding.identity()
        || exit_contract.contract().selected != selected
        || exit_contract.contract().post_allocation_manifest != post.identity
        || exit_contract.contract().post_allocation_machine != machine_receipt.identity()
        || exit_contract.contract().pre_layout != encoding.identity()
        || exit_contract.contract().resolved_layout != layout.identity()
        || !matches!(
            exit_contract.contract().layout_custody,
            WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
        )
    {
        return Err(AllocationRecoveryFunctionRelativeRealizationError::RootMismatch);
    }
    let empty = OptimizationSelections::default().identity();
    let unavailable = FunctionRelativeOptimizationUnavailableData::Unavailable;
    let record = FunctionRelativeOptimizationRealizationManifest {
        identity: omega_optimization_core::FunctionRelativeOptimizationRealizationManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1,
        selections: selections.identity(),
        selected_lowering_selections: empty,
        selected_lowering_completion: None,
        allocation_recovery_selections: selections.for_phase(OptimizationExecutionPhase::AllocationRecovery).identity(),
        post_allocation_machine_selections: empty,
        function_relative_layout_selections: empty,
        pre_physical_manifest: post.pre_physical,
        post_allocation_manifest: post.identity,
        selected,
        pre_allocation_machine_effects: machine.effects().receipt().identity(),
        post_allocation_machine: machine_receipt.identity(),
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
        statistics: function_relative_statistics(layout).map_err(AllocationRecoveryFunctionRelativeRealizationError::Manifest)?,
        frame: FunctionRelativeFrameDisposition::Unavailable,
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
