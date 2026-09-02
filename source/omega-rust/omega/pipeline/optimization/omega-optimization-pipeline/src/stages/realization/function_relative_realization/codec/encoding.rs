use super::super::model::FunctionRelativeOptimizationRealizationManifest;
use super::super::prelude::*;
use super::super::{
    FunctionRelativeOptimizationRealizationScope, FunctionRelativeOptimizationRealizationStage,
    FunctionRelativeOptimizationUnavailableData,
};
use super::post_allocation::encode_optional_custody;
use super::target::encode_target;

pub(super) fn encode_manifest_content(
    manifest: &FunctionRelativeOptimizationRealizationManifest,
) -> Vec<u8> {
    let mut canonical = Vec::new();
    canonical.push(match manifest.stage {
        FunctionRelativeOptimizationRealizationStage::ValidatedFunctionRelativeSelectedFormsAndWholeFunctionExitV1 => 1,
    });
    canonical.extend_from_slice(&manifest.selections.bytes());
    canonical.extend_from_slice(&manifest.selected_lowering_selections.bytes());
    match manifest.selected_lowering_completion {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&manifest.allocation_recovery_selections.bytes());
    for identity in [
        manifest.post_allocation_machine_selections.bytes(),
        manifest.function_relative_layout_selections.bytes(),
        manifest.pre_physical_manifest.bytes(),
        manifest.post_allocation_manifest.bytes(),
        manifest.selected.bytes(),
        manifest.pre_allocation_machine_effects.bytes(),
        manifest.post_allocation_machine.bytes(),
        manifest.baseline_pre_layout.bytes(),
        manifest.pre_layout.bytes(),
        manifest.baseline_resolved_layout.bytes(),
        manifest.resolved_layout.bytes(),
    ] {
        canonical.extend_from_slice(&identity);
    }
    match manifest.x86_branch_relaxation {
        Some(identity) => {
            canonical.push(1);
            canonical.extend_from_slice(&identity.bytes());
        }
        None => canonical.push(0),
    }
    encode_optional_custody(
        &mut canonical,
        manifest.post_allocation_machine_optimization,
    );
    canonical.extend_from_slice(&manifest.whole_function_exit_contract.bytes());
    encode_target(&mut canonical, manifest.target);
    canonical.push(match manifest.layout_policy {
        SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 => 1,
        SelectedFunctionLayoutPolicy::SingleEntryBlockV1 => 2,
        SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1 => 3,
        SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1 => 4,
        SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1 => 5,
    });
    canonical.push(match manifest.scope {
        FunctionRelativeOptimizationRealizationScope::FunctionRelativeFragmentsWithValidatedWholeFunctionExitV1 => 1,
    });
    for value in [
        manifest.statistics.functions,
        manifest.statistics.blocks,
        manifest.statistics.instructions,
        manifest.statistics.bytes,
        manifest.statistics.resolved_conditional_branches,
        manifest.statistics.structural_unit_functions,
        manifest.statistics.structural_unit_blocks,
        manifest.statistics.structural_unit_instructions,
        manifest.statistics.structural_unit_bytes,
        manifest.statistics.unresolved_internal_machine_fixups,
    ] {
        canonical.extend_from_slice(&value.to_le_bytes());
    }
    for unavailable in [
        manifest.frame,
        manifest.machine_emission,
        manifest.section_placement,
        manifest.symbols,
        manifest.object_relocations,
        manifest.executable_image,
        manifest.installation,
        manifest.publication,
    ] {
        canonical.push(match unavailable {
            FunctionRelativeOptimizationUnavailableData::Unavailable => 1,
        });
    }
    canonical
}
