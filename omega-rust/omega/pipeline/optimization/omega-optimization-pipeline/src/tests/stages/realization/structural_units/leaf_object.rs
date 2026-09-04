use crate::tests::*;

#[test]
fn structural_extent_unit_leaf_reaches_canonical_object_artifact() {
    let (semantic, proof) = structural_extent_unit_leaf_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .expect("the honest two-Extent Unit leaf must pass PSI optimization custody");
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::uefi_x64(),
        &[],
    )
    .expect("the structural Unit leaf must reach physical custody");
    let StagedOptimizedVerifiedPhysicalPipeline::PhysicalIdentity { homes, .. } = physical else {
        panic!("the PSI-only request must retain baseline structural physical custody")
    };

    let realization = stage_optimized_structural_unit_function_relative_realization(homes)
        .expect("the call-free structural Unit leaf must own function-relative custody");
    let exit = realization.exit_contract().contract();
    assert_eq!(
        exit.policy,
        WholeFunctionExitPolicy::MicrosoftX64FramelessStructuralUnitLeafV1
    );
    assert!(exit.functions.is_empty());
    assert_eq!(exit.structural_unit_functions.len(), 1);
    assert_eq!(
        exit.structural_unit_functions[0].machine,
        MachineId::new(3_602).unwrap()
    );
    assert!(exit.structural_unit_functions[0].call.is_none());
    assert_eq!(exit.structural_unit_functions[0].body_stack_delta, 0);
    assert!(
        exit.structural_unit_functions[0]
            .modified_callee_saved_units
            .is_empty()
    );
    assert_eq!(
        exit.structural_unit_functions[0].returned.value,
        WholeFunctionReturnValueEvidence::UnitV1
    );
    let realization_manifest = realization.manifest().record();
    assert_eq!(realization_manifest.statistics.structural_unit_functions, 1);
    assert_eq!(realization_manifest.statistics.structural_unit_blocks, 1);
    assert_eq!(
        realization_manifest.statistics.structural_unit_instructions,
        1
    );
    assert_eq!(realization_manifest.statistics.structural_unit_bytes, 1);
    assert_eq!(
        realization_manifest
            .statistics
            .unresolved_internal_machine_fixups,
        0
    );

    let fragments = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::StructuralUnit(Box::new(realization)),
    )
    .expect("the leaf must emit one relocation-free structural fragment");
    let fragment_manifest = fragments.manifest().record();
    assert_eq!(
        fragment_manifest.stage,
        FunctionFragmentEmissionStage::ValidatedRelocationFreeFunctionFragmentsV1
    );
    assert_eq!(
        fragment_manifest.source_kind,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1
    );
    assert_eq!(fragment_manifest.statistics.structural_unit_functions, 1);
    assert_eq!(
        fragment_manifest
            .statistics
            .structural_unit_instruction_spans,
        1
    );
    assert_eq!(fragment_manifest.statistics.structural_unit_bytes, 1);
    assert_eq!(
        fragment_manifest
            .statistics
            .unresolved_internal_machine_fixups,
        0
    );
    assert_eq!(fragments.fragments().structural_unit_functions.len(), 1);
    let leaf = &fragments.fragments().structural_unit_functions[0];
    assert_eq!(leaf.bytes, [0xc3]);
    assert!(leaf.block.call.is_none());
    assert_eq!(leaf.block.return_instruction.offset, 0);

    let text = stage_optimized_relocation_free_text_section(fragments)
        .expect("the call-free structural leaf must place without fixup resolution");
    assert_eq!(text.text_section().bytes, [0xc3]);
    assert_eq!(text.text_section().functions.len(), 1);
    assert!(
        text.text_section()
            .resolved_internal_machine_calls
            .is_empty()
    );
    let text_manifest = text.manifest().record();
    assert_eq!(
        text_manifest.source_kind,
        FunctionFragmentEmissionSourceKind::StructuralUnitV1
    );
    assert_eq!(text_manifest.statistics.structural_unit_functions, 1);
    assert_eq!(text_manifest.statistics.structural_unit_bytes, 1);
    assert_eq!(text_manifest.statistics.source_internal_machine_fixups, 0);
    assert_eq!(text_manifest.statistics.resolved_internal_machine_fixups, 0);
    assert_eq!(
        text_manifest.statistics.remaining_internal_machine_fixups,
        0
    );

    let object = stage_optimized_relocation_free_object_container(text)
        .expect("the leaf text must enter a relocation-free object container");
    assert_eq!(object.object().text_section.bytes, [0xc3]);
    assert_eq!(object.object().symbols.len(), 1);
    assert_eq!(object.object().symbols[0].section_offset, 0);
    assert_eq!(object.object().symbols[0].byte_count, 1);
    assert_eq!(object.object().relocation_record_count, 0);

    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .expect("the leaf object must retain the exact canonical semantic/proof join");
    assert_eq!(
        artifact.artifact().semantic_entry,
        MachineId::new(3_602).unwrap()
    );
    assert_eq!(artifact.artifact().statistics.text_bytes, 1);
    assert_eq!(artifact.artifact().statistics.function_symbols, 1);
    assert_eq!(artifact.artifact().statistics.relocation_records, 0);
    validate_optimized_object_artifact(&artifact).unwrap();
}
