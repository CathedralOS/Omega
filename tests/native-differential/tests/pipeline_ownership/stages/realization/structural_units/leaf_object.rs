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
    let source = physical.into_function_fragment_emission_source();
    let exit = source.exit_contract().contract();
    assert_eq!(exit.functions.len(), 1);
    assert_eq!(exit.functions[0].machine, MachineId::new(3_602).unwrap());
    assert_eq!(exit.functions[0].body_stack_delta, 0);
    assert!(
        exit.functions[0]
            .returns
            .iter()
            .all(|returned| returned.value == WholeFunctionReturnValueEvidence::UnitV1)
    );
    let framed = source.frame_layout().is_some();
    assert_eq!(
        framed,
        matches!(
            exit.frame,
            WholeFunctionFrameDisposition::CanonicalFixedFrameV1 { .. }
        )
    );
    let fragments = stage_optimized_function_fragment_emission(source).unwrap();
    assert_eq!(fragments.fragments().functions.len(), 1);
    assert_eq!(
        fragments
            .manifest()
            .record()
            .statistics
            .unresolved_internal_machine_fixups,
        0
    );
    let check_text = |text: &machine_code::RelocationFreeTextSectionPlacement,
                      manifest: &FunctionFragmentTextSectionManifest| {
        assert_eq!(text.functions.len(), 1);
        assert!(text.resolved_internal_machine_calls.is_empty());
        assert_eq!(manifest.statistics.functions, 1);
        assert_eq!(manifest.statistics.remaining_internal_machine_fixups, 0);
    };
    let object = if framed {
        let applied = stage_function_fragment_frame_application(fragments).unwrap();
        let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
        check_text(text.text_section(), text.manifest().record());
        validate_optimized_fixed_frame_text_section(&text).unwrap();
        stage_optimized_relocation_free_object_container(text).unwrap()
    } else {
        let text = stage_optimized_relocation_free_text_section(fragments).unwrap();
        check_text(text.text_section(), text.manifest().record());
        validate_optimized_relocation_free_text_section(&text).unwrap();
        stage_optimized_relocation_free_object_container(text).unwrap()
    };
    assert_eq!(object.object().symbols.len(), 1);
    assert_eq!(object.object().symbols[0].section_offset, 0);
    assert_eq!(
        object.object().symbols[0].byte_count,
        object.object().text_section.byte_count
    );
    assert_eq!(object.object().relocation_record_count, 0);
    let object = std::sync::Arc::new(object);
    let published = image_emission::build_function_fragment_object_artifact(object.clone())
        .expect("the shared structural leaf must publish without invented stack homes");
    image_emission::validate_function_fragment_object_artifact(&object, &published)
        .expect("structural object publication must independently replay");
    let image = image_emission::emit_executable_image(&published, 10)
        .expect("the structural object must retain its ABI through image publication");
    let installation = image_emission::build_installation_record(
        &image,
        semantic_vocabulary::ProfileDecisionId::new(1).unwrap(),
    )
    .expect("structural ABI installation custody");
    let bytes = image_emission::encode_installation_record(&installation)
        .expect("structural ABI installation encoding");
    let decoded = image_emission::decode_installation_record(&bytes)
        .expect("structural ABI installation decoding");
    assert_eq!(decoded, installation);
    image_emission::validate_installation_record(&decoded, &image)
        .expect("structural ABI installation replay");

    drop(image);
    drop(published);
    let object = std::sync::Arc::try_unwrap(object)
        .expect("completed image publication releases the original staged object");
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .expect("the leaf object must retain the exact canonical semantic/proof join");
    assert_eq!(
        artifact.artifact().semantic_entry,
        MachineId::new(3_602).unwrap()
    );
    assert!(artifact.artifact().statistics.text_bytes > 0);
    assert_eq!(artifact.artifact().statistics.function_symbols, 1);
    assert_eq!(artifact.artifact().statistics.relocation_records, 0);
    validate_optimized_object_artifact(&artifact).unwrap();
}
