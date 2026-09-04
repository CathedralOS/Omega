use omega_machine_code::FunctionFragmentEmissionPlan;
use omega_object_file::RelocationFreeTextSectionPlacement;
use omega_optimization_core::FunctionFragmentTextSectionManifestIdentity;

use crate::{StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission};

use super::{
    FunctionFragmentTextSectionManifest, FunctionFragmentTextSectionSourceCustody,
    FunctionFragmentTextSectionStage, FunctionFragmentTextSectionStatistics,
    FunctionFragmentTextSectionUnavailableData, RelocationFreeTextSectionPlacementError,
    StagedFixedFrameTextSectionCustodyReceipt, StagedRelocationFreeTextSectionCustodyReceipt,
    ValidatedFunctionFragmentTextSectionManifest,
    placement::{place_fixed_frame_fragments, place_fragments, usize_to_u64},
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
    source_manifest: &crate::FunctionFragmentEmissionManifest,
    stage: FunctionFragmentTextSectionStage,
    source_custody: FunctionFragmentTextSectionSourceCustody,
    text_section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<ValidatedFunctionFragmentTextSectionManifest, RelocationFreeTextSectionPlacementError> {
    let statistics = statistics(text_section, fragments)?;
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
    Ok(ValidatedFunctionFragmentTextSectionManifest { record })
}

fn statistics(
    section: &RelocationFreeTextSectionPlacement,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<FunctionFragmentTextSectionStatistics, RelocationFreeTextSectionPlacementError> {
    let mut result = FunctionFragmentTextSectionStatistics::default();
    if fragments.structural_unit_functions.is_empty() {
        result.functions = usize_to_u64(section.functions.len())?;
        result.bytes = section.byte_count;
        for function in &section.functions {
            result.blocks = result
                .blocks
                .checked_add(usize_to_u64(function.blocks.len())?)
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
            for block in &function.blocks {
                result.instruction_spans = result
                    .instruction_spans
                    .checked_add(usize_to_u64(block.instructions.len())?)
                    .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
                for row in &block.instructions {
                    result.zero_byte_instruction_spans = result
                        .zero_byte_instruction_spans
                        .checked_add(u64::from(row.byte_count == 0))
                        .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
                }
            }
        }
        result.source_internal_machine_fixups = fragments
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| instruction.internal_machine_fixup.is_some())
            .count()
            .try_into()
            .map_err(|_| RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.resolved_internal_machine_fixups =
            usize_to_u64(section.resolved_internal_machine_calls.len())?;
        result.remaining_internal_machine_fixups = result
            .source_internal_machine_fixups
            .checked_sub(result.resolved_internal_machine_fixups)
            .ok_or(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups)?;
        if result.remaining_internal_machine_fixups != 0 {
            return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
        }
        return Ok(result);
    }
    if !fragments.functions.is_empty()
        || section.functions.len() != fragments.structural_unit_functions.len()
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    result.structural_unit_functions = usize_to_u64(fragments.structural_unit_functions.len())?;
    for function in &fragments.structural_unit_functions {
        result.structural_unit_blocks = result
            .structural_unit_blocks
            .checked_add(1)
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_bytes = result
            .structural_unit_bytes
            .checked_add(function.byte_count)
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_instruction_spans = result
            .structural_unit_instruction_spans
            .checked_add(1 + u64::from(function.block.call.is_some()))
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        result.structural_unit_zero_byte_instruction_spans = result
            .structural_unit_zero_byte_instruction_spans
            .checked_add(u64::from(
                function.block.return_instruction.bytes.is_empty(),
            ))
            .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        if let Some(call) = &function.block.call {
            result.structural_unit_zero_byte_instruction_spans = result
                .structural_unit_zero_byte_instruction_spans
                .checked_add(u64::from(call.bytes.is_empty()))
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
            result.source_internal_machine_fixups = result
                .source_internal_machine_fixups
                .checked_add(1)
                .ok_or(RelocationFreeTextSectionPlacementError::StatisticsOverflow)?;
        }
    }
    result.resolved_internal_machine_fixups =
        usize_to_u64(section.resolved_internal_machine_calls.len())?;
    result.remaining_internal_machine_fixups = result
        .source_internal_machine_fixups
        .checked_sub(result.resolved_internal_machine_fixups)
        .ok_or(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups)?;
    if result.structural_unit_bytes != section.byte_count
        || result.remaining_internal_machine_fixups != 0
    {
        return Err(RelocationFreeTextSectionPlacementError::UnresolvedInternalMachineFixups);
    }
    Ok(result)
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
