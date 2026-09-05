//! X86 rel8 byte placement, manifest replay, and corruption rejection.

use crate::FunctionFragmentReplayInputs;
use crate::tests::*;

#[test]
fn relocation_free_rel8_text_section_replays_bytes_manifest_and_custody() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_function_relative_layout_for_test()
        .unwrap_or_else(|| panic!("rel8 must complete its direct function-relative realization"));
    let emitted = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::X86Rel8Direct(Box::new(realization)).into(),
    )
    .unwrap();
    let source_fragments = emitted.fragments().identity;
    let source_bytes = emitted.fragments().functions[0].bytes.clone();
    let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    crate::tests::text_placement_checks::direct(&placed);

    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed).unwrap(),
        placed.custody()
    );
    let section = placed.text_section();
    assert_eq!(section.source_fragments, source_fragments);
    assert_eq!(section.section_alignment, 1);
    assert_eq!(section.bytes, source_bytes);
    assert_eq!(section.byte_count, section.bytes.len() as u64);
    assert_eq!(section.functions.len(), 1);
    assert_eq!(section.functions[0].source_function_index, 0);
    assert_eq!(section.functions[0].section_offset, 0);
    assert_eq!(section.semantic_entry_offset, 0);
    let branch = section.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| {
            row.alternative.family
                == selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
        })
        .unwrap();
    assert_eq!(section.bytes[branch.section_offset as usize], 0x75);
    assert_eq!(branch.function_offset, branch.section_offset);
    assert_eq!(branch.byte_count, 2);
    assert_eq!(
        section.relocation_requirements,
        object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
    );

    let record = placed.manifest().record();
    assert_eq!(
        record.source_fragment_manifest,
        placed.source().manifest().record().identity
    );
    assert_eq!(record.fragments, source_fragments);
    assert_eq!(record.text_section, section.identity);
    assert_eq!(record.statistics.padding_bytes, 0);
    assert_eq!(record.statistics.relocation_requirements, 0);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&record.encode()),
        Ok(record.clone())
    );
    let mut trailing = record.encode();
    trailing.push(0);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&trailing),
        Err(FunctionFragmentTextSectionManifestDecodeError::TrailingBytes)
    );
    let mut wrong_version = record.encode();
    wrong_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&wrong_version),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnsupportedVersion(1))
    );
    let mut stale_identity = record.encode();
    stale_identity[12] ^= 1;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&stale_identity),
        Err(FunctionFragmentTextSectionManifestDecodeError::IdentityMismatch)
    );
    let mut unknown_relocation = record.encode();
    let relocation_tag = unknown_relocation.len() - 127;
    unknown_relocation[relocation_tag] = 2;
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&unknown_relocation),
        Err(FunctionFragmentTextSectionManifestDecodeError::UnknownRelocationRequirements(2))
    );
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&record.encode()[..20]),
        Err(FunctionFragmentTextSectionManifestDecodeError::Truncated)
    );

    let original_byte = placed.text_section().bytes[0];
    placed.text_section_mut().bytes[0] ^= 1;
    let corrupted_identity = placed.text_section().recomputed_identity();
    placed.text_section_mut().identity = corrupted_identity;
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed),
        Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
    );
    placed.text_section_mut().bytes[0] = original_byte;
    let restored_identity = placed.text_section().recomputed_identity();
    placed.text_section_mut().identity = restored_identity;
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed).unwrap(),
        placed.custody()
    );

    let original_manifest = placed.manifest().record().clone();
    placed.manifest_mut().record_mut().statistics.padding_bytes = 1;
    let corrupted_manifest = placed.manifest().record().recomputed_identity();
    placed.manifest_mut().record_mut().identity = corrupted_manifest;
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed),
        Err(RelocationFreeTextSectionPlacementError::ManifestMismatch)
    );
    *placed.manifest_mut().record_mut() = original_manifest;
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed).unwrap(),
        placed.custody()
    );
    placed.corrupt_custody_manifest_for_test();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed),
        Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch)
    );
    let current = placed.shared_text_section();
    let manifest = placed.manifest().shared_record();
    assert!(std::ptr::eq(manifest.as_ref(), placed.manifest().record()));
    let identity = current.identity;
    drop(placed);
    assert_eq!(current.recomputed_identity(), identity);
    assert_eq!(current.bytes, source_bytes);
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&manifest.encode()),
        Ok((*manifest).clone())
    );
}
