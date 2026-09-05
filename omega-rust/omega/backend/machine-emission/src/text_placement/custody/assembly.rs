use machine_code::FunctionFragmentEmissionPlan;
use machine_code::RelocationFreeTextSectionPlacement;
use optimization_core::FunctionFragmentTextSectionManifestIdentity;

use crate::{StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission};

use super::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionSourceCustody,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionUnavailableData,
    RelocationFreeTextSectionPlacementError, StagedFixedFrameTextSectionCustodyReceipt,
    StagedRelocationFreeTextSectionCustodyReceipt, ValidatedFunctionFragmentTextSectionManifest,
    placement::{place_fixed_frame_fragments, place_fragments},
};

pub(super) fn compute(
    source: &StagedOptimizedFunctionFragmentEmission,
) -> Result<
    (
        RelocationFreeTextSectionPlacement,
        ValidatedFunctionFragmentTextSectionManifest,
    ),
    RelocationFreeTextSectionPlacementError,
> {
    let fragments = source.fragments();
    let source_manifest = source.manifest().record();
    let text_section = place_fragments(source)?;
    let manifest = manifest(
        source_manifest,
        FunctionFragmentTextSectionStage::ValidatedRelocationFreeTextSectionPlacementV1,
        FunctionFragmentTextSectionSourceCustody::DirectFragmentEmissionV1,
        &text_section,
        fragments,
    )?;
    Ok((text_section, manifest))
}

pub(super) fn compute_fixed_frame(
    source: &StagedFunctionFragmentFrameApplication,
) -> Result<
    (
        RelocationFreeTextSectionPlacement,
        ValidatedFunctionFragmentTextSectionManifest,
    ),
    RelocationFreeTextSectionPlacementError,
> {
    let fragments = source.fragments();
    let source_manifest = source.source().manifest().record();
    let text_section = place_fixed_frame_fragments(source)?;
    let manifest = manifest(
        source_manifest,
        FunctionFragmentTextSectionStage::ValidatedFixedFrameInternalCallTextSectionPlacementV1,
        FunctionFragmentTextSectionSourceCustody::FixedFrameApplicationV1 {
            application: source.receipt().identity(),
        },
        &text_section,
        fragments,
    )?;
    Ok((text_section, manifest))
}

fn manifest(
    source_manifest: &machine_code::FunctionFragmentEmissionManifest,
    stage: FunctionFragmentTextSectionStage,
    source_custody: FunctionFragmentTextSectionSourceCustody,
    text_section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<ValidatedFunctionFragmentTextSectionManifest, RelocationFreeTextSectionPlacementError> {
    let statistics = crate::text_section_statistics(text_section, fragments)?;
    let unavailable = FunctionFragmentTextSectionUnavailableData::Unavailable;
    let mut record = FunctionFragmentTextSectionManifest {
        identity: FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"pending"),
        stage,
        source_custody,
        source_kind: source_manifest.source_kind,
        source_fragment_manifest: source_manifest.identity,
        source_realization: source_manifest.source_realization,
        selections: source_manifest.selections,
        psi: source_manifest.psi,
        fuel_schedule: source_manifest.fuel_schedule,
        selected: source_manifest.selected,
        post_allocation_manifest: source_manifest.post_allocation_manifest,
        post_allocation_machine: source_manifest.post_allocation_machine,
        final_pre_layout: source_manifest.final_pre_layout,
        final_resolved_layout: source_manifest.final_resolved_layout,
        whole_function_exit_contract: source_manifest.whole_function_exit_contract,
        fragments: fragments.identity,
        target: source_manifest.target,
        semantic_entry: text_section.semantic_entry,
        semantic_entry_offset: text_section.semantic_entry_offset,
        placement_policy: text_section.policy,
        text_section: text_section.identity,
        relocation_requirements: text_section.relocation_requirements,
        statistics,
        symbols: unavailable,
        object_container: unavailable,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentTextSectionManifest {
        record: std::sync::Arc::new(record),
    })
}

pub(super) fn fixed_frame_receipt(
    source: &StagedFunctionFragmentFrameApplication,
    manifest: &ValidatedFunctionFragmentTextSectionManifest,
    section: &RelocationFreeTextSectionPlacement,
) -> StagedFixedFrameTextSectionCustodyReceipt {
    StagedFixedFrameTextSectionCustodyReceipt {
        frame_application: source.receipt().identity(),
        fragments: section.source_fragments,
        text_section: section.identity,
        manifest: manifest.record.identity,
    }
}

pub(super) fn receipt(
    manifest: &ValidatedFunctionFragmentTextSectionManifest,
    section: &RelocationFreeTextSectionPlacement,
) -> StagedRelocationFreeTextSectionCustodyReceipt {
    StagedRelocationFreeTextSectionCustodyReceipt {
        source_fragment_manifest: manifest.record.source_fragment_manifest,
        fragments: section.source_fragments,
        text_section: section.identity,
        manifest: manifest.record.identity,
    }
}
