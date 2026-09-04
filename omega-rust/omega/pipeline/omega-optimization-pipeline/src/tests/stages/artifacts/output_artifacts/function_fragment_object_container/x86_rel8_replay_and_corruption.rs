//! X86 rel8 object reconstruction, replay, and corruption rejection.

use crate::tests::*;

fn object_local_symbol_count(object: &omega_object_file::RelocationFreeObjectPlan) -> usize {
    object
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.linkage == omega_object_file::RelocationFreeObjectSymbolLinkage::ObjectLocalV1
        })
        .count()
}

#[test]
fn relocation_free_rel8_object_container_reconstructs_replays_and_rejects_corruption() {
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
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let mut staged = stage_optimized_relocation_free_object_container(placed).unwrap();

    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged).unwrap(),
        staged.custody()
    );
    let object = staged.object();
    assert_eq!(
        object.text_section.bytes,
        staged.source().text_section().bytes
    );
    assert_eq!(object.text_section.name, ".text");
    assert_eq!(object.text_section.alignment, 1);
    assert_eq!(object.relocation_record_count, 0);
    assert_eq!(object.symbols.len(), object_local_symbol_count(object));
    assert_eq!(object.symbols.len(), 1);
    let entry = &object.symbols[0];
    assert_eq!(entry.symbol, object.semantic_entry_symbol);
    assert_eq!(entry.machine, object.semantic_entry);
    assert_eq!(
        entry.name,
        format!("__omega_terminal_machine_{}", entry.machine.get())
    );
    assert_ne!(entry.name, "main");
    assert_ne!(entry.name, "_main");
    assert_eq!(entry.section_offset, 0);
    assert_eq!(entry.byte_count, object.text_section.byte_count);
    assert_eq!(
        omega_object_file::decode_relocation_free_object(&staged.container().bytes),
        Ok(object.clone())
    );
    let record = staged.manifest().record();
    assert_eq!(
        FunctionFragmentObjectContainerManifest::decode(&record.encode()),
        Ok(record.clone())
    );
    assert_eq!(record.statistics.sections, 1);
    assert_eq!(record.statistics.external_symbols, 0);
    assert_eq!(record.statistics.relocation_records, 0);
    assert_eq!(record.statistics.text_bytes, object.text_section.byte_count);

    let original_object = staged.object().clone();
    staged.object_mut().symbols[0].name.push_str("_corrupt");
    let corrupted_object_identity = staged.object().recomputed_identity().unwrap();
    staged.object_mut().identity = corrupted_object_identity;
    assert!(matches!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::InvalidObject(_))
            | Err(RelocationFreeObjectContainerError::ArtifactMismatch)
    ));
    *staged.object_mut() = original_object;

    let original_container = staged.container().clone();
    staged.container_mut().bytes[0] ^= 1;
    let corrupted_container_identity =
        omega_optimization_core::RelocationFreeObjectContainerIdentity::from_canonical_bytes(
            &staged.container().bytes,
        );
    staged.container_mut().identity = corrupted_container_identity;
    assert!(matches!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::InvalidContainer(_))
            | Err(RelocationFreeObjectContainerError::ContainerMismatch)
    ));
    *staged.container_mut() = original_container;

    let original_manifest = staged.manifest().record().clone();
    staged
        .manifest_mut()
        .record_mut()
        .statistics
        .external_symbols = 1;
    let corrupted_manifest_identity = staged.manifest().record().recomputed_identity();
    staged.manifest_mut().record_mut().identity = corrupted_manifest_identity;
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::ManifestMismatch)
    );
    *staged.manifest_mut().record_mut() = original_manifest;
    staged.corrupt_custody_manifest_for_test();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::ReceiptMismatch)
    );
}
