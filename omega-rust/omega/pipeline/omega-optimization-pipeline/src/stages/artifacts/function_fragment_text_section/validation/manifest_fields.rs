//! Check publication claims against admitted inputs and the independently checked section.
use super::super::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionSourceCustody,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionUnavailableData,
    RelocationFreeTextSectionPlacementError,
};
use omega_machine_code::{
    FunctionFragmentEmissionManifest, FunctionFragmentEmissionPlan,
    RelocationFreeTextSectionPlacement,
};

pub(super) fn check(
    source: &FunctionFragmentEmissionManifest,
    stage: FunctionFragmentTextSectionStage,
    custody: FunctionFragmentTextSectionSourceCustody,
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
    candidate: &FunctionFragmentTextSectionManifest,
) -> Result<(), RelocationFreeTextSectionPlacementError> {
    if candidate.identity != candidate.recomputed_identity()
        || candidate.stage != stage
        || candidate.source_custody != custody
        || candidate.source_fragment_manifest != source.identity
        || candidate.source_kind != source.source_kind
        || candidate.source_realization != source.source_realization
        || candidate.selections != source.selections
        || candidate.psi != source.psi
        || candidate.fuel_schedule != source.fuel_schedule
        || candidate.selected != source.selected
        || candidate.post_allocation_manifest != source.post_allocation_manifest
        || candidate.post_allocation_machine != source.post_allocation_machine
        || candidate.final_pre_layout != source.final_pre_layout
        || candidate.final_resolved_layout != source.final_resolved_layout
        || candidate.whole_function_exit_contract != source.whole_function_exit_contract
        || candidate.target != source.target
        || candidate.semantic_entry != section.semantic_entry
        || candidate.semantic_entry_offset != section.semantic_entry_offset
        || candidate.relocation_requirements != section.relocation_requirements
        || candidate.fragments != fragments.identity
        || candidate.placement_policy != section.policy
        || candidate.text_section != section.identity
        || candidate.statistics
            != omega_machine_emission::text_section_statistics(section, fragments)?
        || candidate.symbols != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.object_container != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.external_entry_bridge
            != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.executable_image != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.installation != FunctionFragmentTextSectionUnavailableData::Unavailable
        || candidate.publication != FunctionFragmentTextSectionUnavailableData::Unavailable
    {
        return Err(RelocationFreeTextSectionPlacementError::ManifestMismatch);
    }
    Ok(())
}
