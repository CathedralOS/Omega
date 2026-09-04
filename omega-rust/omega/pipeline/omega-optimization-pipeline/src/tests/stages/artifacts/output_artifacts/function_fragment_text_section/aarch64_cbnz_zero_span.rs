//! AArch64 CBNZ zero-span preservation and alignment.

use crate::tests::*;

#[test]
fn relocation_free_cbnz_text_section_preserves_zero_span_and_alignment() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (physical)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| panic!("CBNZ must complete its direct function-relative realization"));
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let section = placed.text_section();
    assert_eq!(section.section_alignment, 4);
    assert_eq!(section.byte_count % 4, 0);
    assert!(
        section
            .functions
            .iter()
            .all(|function| function.section_offset % 4 == 0 && function.byte_count % 4 == 0)
    );
    let rows = section.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect::<Vec<_>>();
    let compare = rows
        .iter()
        .find(|row| {
            row.alternative.family
                == omega_selected_instructions::MachineAlternativeFamily::CompareI64Zero
        })
        .unwrap();
    let branch = rows
        .iter()
        .find(|row| {
            row.alternative.family
                == omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
        })
        .unwrap();
    assert_eq!(compare.byte_count, 0);
    assert_eq!(compare.function_offset, branch.function_offset);
    assert_eq!(compare.section_offset, branch.section_offset);
    assert_eq!(branch.byte_count, 4);
    assert_eq!(
        u32::from_le_bytes(
            section.bytes[branch.section_offset as usize..branch.section_offset as usize + 4]
                .try_into()
                .unwrap()
        ) & 0xff00_0000,
        0xb500_0000
    );
    assert_eq!(
        placed
            .manifest()
            .record()
            .statistics
            .zero_byte_instruction_spans,
        1
    );

    let compare = placed.text_section_mut().functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.byte_count == 0)
        .unwrap();
    compare.byte_count = 4;
    let corrupted_identity = placed.text_section().recomputed_identity();
    placed.text_section_mut().identity = corrupted_identity;
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed),
        Err(RelocationFreeTextSectionPlacementError::ArtifactMismatch)
    );
}
