use crate::tests::*;

#[test]
fn active_resident_rematerialization_emits_relocation_free_fragments_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let realization = staged_active_resident_function_relative_realization(target);
        let rematerialization = realization.source().pre_layout().source();
        let action = rematerialization.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .expect("the admitted source must retain its rematerialization action");
        let fresh = action.fresh_materialize;
        let transformed_selected = rematerialization
            .rematerialization()
            .receipt()
            .transformed_selected();
        let transformed_homes = rematerialization.homes().receipt();
        let register_environment = rematerialization
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .identity();
        let optimized_source = rematerialization
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .optimized_target()
            .optimized();
        let pre_physical = optimized_source.pre_physical_manifest().record().identity;
        let verified_input = optimized_source.verified_input().clone();
        let source_manifest = realization.manifest().record().clone();
        let mut emitted = stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::ActiveResidentRematerialization(
                Box::new(realization),
            ),
        )
        .unwrap();

        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );
        assert_eq!(
            emitted.source().selected_plan().psi,
            emitted.fragments().psi
        );
        assert_eq!(
            emitted.source().register_homes().receipt(),
            transformed_homes
        );
        assert_eq!(
            emitted.source().register_environment().identity(),
            register_environment
        );
        assert_eq!(
            emitted.source().pre_physical_manifest().record().identity,
            pre_physical
        );
        assert_eq!(emitted.source().verified_input(), &verified_input);
        assert_eq!(emitted.fragments().selected, transformed_selected);
        assert_eq!(emitted.manifest().record().selected, transformed_selected);
        assert_eq!(
            emitted.manifest().record().source_realization,
            source_manifest.identity
        );
        assert_eq!(
            source_manifest.allocation_recovery_selections,
            source_manifest.selections
        );
        assert_eq!(
            emitted.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
        );

        let fresh_span = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == fresh)
            .expect("the fresh materialization must have an emitted instruction span");
        assert_eq!(
            fresh_span.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_span.bytes.is_empty());

        let branch = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.branch.is_some())
            .expect("the conditional source must retain one resolved branch");
        match target.architecture {
            omega_target::Architecture::X86_64 => {
                assert_eq!(&branch.bytes[..2], [0x0f, 0x85]);
                assert_eq!(branch.bytes.len(), 6);
            }
            omega_target::Architecture::Aarch64 => {
                let instruction = u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap());
                assert_eq!(instruction & 0xff00_001f, 0x5400_0001);
                assert_eq!(branch.bytes.len(), 4);
            }
        }

        let record = emitted.manifest().record();
        let encoded = record.encode();
        assert_eq!(&encoded[8..12], &7_u32.to_le_bytes());
        assert_eq!(encoded[45], 3);
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&encoded),
            Ok(record.clone())
        );
        let mut unknown_source = encoded;
        unknown_source[45] = 7;
        assert_eq!(
            FunctionFragmentEmissionManifest::decode(&unknown_source),
            Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(7))
        );

        let original_fresh_byte = emitted.fragments().functions[0]
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0];
        emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0] ^= 1;
        emitted.fragments_mut().identity = emitted.fragments().recomputed_identity();
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted),
            Err(FunctionFragmentEmissionError::ArtifactMismatch)
        );
        emitted.fragments_mut().functions[0]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.instruction == fresh)
            .unwrap()
            .bytes[0] = original_fresh_byte;
        emitted.fragments_mut().identity = emitted.fragments().recomputed_identity();
        assert_eq!(
            validate_optimized_function_fragment_emission(&emitted).unwrap(),
            emitted.custody()
        );

        let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
        assert_eq!(
            validate_optimized_relocation_free_text_section(&placed).unwrap(),
            placed.custody()
        );
        assert_eq!(
            placed.text_section().relocation_requirements,
            omega_object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        );
        assert_eq!(
            placed
                .manifest()
                .record()
                .statistics
                .relocation_requirements,
            0
        );
        assert_eq!(
            placed.manifest().record().source_kind,
            FunctionFragmentEmissionSourceKind::ActiveResidentImmediateU64MultiUseRematerializationV1
        );
        let text_encoded = placed.manifest().record().encode();
        assert_eq!(&text_encoded[8..12], &7_u32.to_le_bytes());
        assert_eq!(text_encoded[45], 3);
        assert_eq!(
            FunctionFragmentTextSectionManifest::decode(&text_encoded),
            Ok(placed.manifest().record().clone())
        );
    }
}

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
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct function-relative realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let source_fragments = emitted.fragments().identity;
    let source_bytes = emitted.fragments().functions[0].bytes.clone();
    let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();

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
                == omega_selected_instructions::MachineAlternativeFamily::ConditionalBranchNonZero
        })
        .unwrap();
    assert_eq!(section.bytes[branch.section_offset as usize], 0x75);
    assert_eq!(branch.function_offset, branch.section_offset);
    assert_eq!(branch.byte_count, 2);
    assert_eq!(
        section.relocation_requirements,
        omega_object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
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
    placed.corrupt_custody_for_test();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&placed),
        Err(RelocationFreeTextSectionPlacementError::ReceiptMismatch)
    );
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
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct function-relative realization")
    };
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
    staged.corrupt_custody_for_test();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged),
        Err(RelocationFreeObjectContainerError::ReceiptMismatch)
    );
}

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
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("CBNZ must complete its direct function-relative realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let mut placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let section = placed.text_section();
    assert_eq!(section.section_alignment, 4);
    assert_eq!(section.byte_count % 4, 0);
    assert!(section
        .functions
        .iter()
        .all(|function| function.section_offset % 4 == 0 && function.byte_count % 4 == 0));
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

#[test]
fn relocation_free_cbnz_object_container_retains_zero_span_source_and_private_entry() {
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
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("CBNZ must complete its direct function-relative realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let staged = stage_optimized_relocation_free_object_container(placed).unwrap();
    assert_eq!(staged.object().text_section.alignment, 4);
    assert_eq!(
        staged.object().text_section.bytes,
        staged.source().text_section().bytes
    );
    assert!(staged.source().text_section().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| instruction.byte_count == 0));
    assert_eq!(
        staged.object().symbols[0].role,
        omega_object_file::RelocationFreeObjectSymbolRole::SemanticEntryV1
    );
    assert_eq!(
        validate_optimized_relocation_free_object_container(&staged).unwrap(),
        staged.custody()
    );
}

#[test]
fn optimized_rel8_object_artifact_binds_replays_and_reports_without_authority() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let terminal = canonical_artifact(&semantic, &proof);
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
    let physical_report = optimization_pipeline_report(&physical);
    assert_eq!(physical_report.function_fragment(), None);
    assert_eq!(physical_report.text_section(), None);
    assert_eq!(physical_report.object_container(), None);
    assert_eq!(physical_report.object_artifact(), None);
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();
    let mut staged = stage_validated_optimized_object_artifact(terminal, object).unwrap();

    assert_eq!(
        validate_optimized_object_artifact(&staged).unwrap(),
        staged.custody()
    );
    let artifact = staged.artifact();
    assert_eq!(artifact.psi, staged.source().object().psi);
    assert_eq!(
        artifact.semantic_entry,
        staged.source().object().semantic_entry
    );
    assert_eq!(artifact.statistics.relocation_records, 0);
    assert_eq!(
        artifact.pre_physical_manifest,
        staged
            .source()
            .source()
            .source()
            .function_relative_manifest()
            .record()
            .pre_physical_manifest
    );
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&artifact.encode()),
        Ok(artifact.clone())
    );
    let manifest = staged.manifest().record();
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&manifest.encode()),
        Ok(manifest.clone())
    );
    assert_eq!(
        manifest.external_entry_bridge,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.executable_image,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.installation,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );
    assert_eq!(
        manifest.publication,
        OptimizedObjectArtifactUnavailableData::Unavailable
    );

    let artifact_identity = artifact.identity;
    let object_bytes = staged.source().container().bytes.clone();
    let report = optimization_pipeline_report_from_object_artifact(&staged);
    assert_eq!(
        report.render_human_text(OptimizationReportRequest::Suppressed),
        None
    );
    let rendered = report
        .render_human_text(OptimizationReportRequest::EmitHumanText)
        .unwrap();
    assert!(rendered.contains("[optimized Omega object artifact]"));
    assert!(rendered.contains("publication: unavailable"));
    assert_eq!(staged.artifact().identity, artifact_identity);
    assert_eq!(staged.source().container().bytes, object_bytes);
    assert_eq!(
        report.function_fragment().unwrap().identity,
        artifact.function_fragment_manifest
    );
    assert_eq!(
        report.text_section().unwrap().identity,
        artifact.text_section_manifest
    );
    assert_eq!(
        report.object_container().unwrap().identity,
        artifact.object_container_manifest
    );
    assert_eq!(
        report.object_artifact().unwrap().artifact,
        artifact.identity
    );

    let original_artifact = staged.artifact().clone();
    staged.artifact_mut().statistics.relocation_records = 1;
    let corrupted_artifact_identity = staged.artifact().recomputed_identity();
    staged.artifact_mut().identity = corrupted_artifact_identity;
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ArtifactMismatch)
    );
    *staged.artifact_mut() = original_artifact;

    let original_manifest = staged.manifest().record().clone();
    staged
        .manifest_mut()
        .record_mut()
        .statistics
        .function_symbols += 1;
    let corrupted_manifest_identity = staged.manifest().record().recomputed_identity();
    staged.manifest_mut().record_mut().identity = corrupted_manifest_identity;
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ManifestMismatch)
    );
    *staged.manifest_mut().record_mut() = original_manifest;
    staged.corrupt_custody_for_test();
    assert_eq!(
        validate_optimized_object_artifact(&staged),
        Err(OptimizedObjectArtifactError::ReceiptMismatch)
    );
}

#[test]
fn optimized_cbnz_object_artifact_retains_zero_span_and_rejects_detached_proof() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1])
            .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("CBNZ must complete its direct realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();

    let module = psi_terminal_codec::decode_module(&semantic).unwrap();
    let mut detached_proof = psi_terminal_codec::decode_proof_bundle(&proof).unwrap();
    detached_proof.evidence.pop();
    let detached =
        psi_terminal_codec::CanonicalTerminalArtifact::from_parts(&module, &detached_proof, None)
            .unwrap();
    assert!(matches!(
        stage_validated_optimized_object_artifact(detached, object),
        Err(OptimizedObjectArtifactError::ProofMismatch)
    ));

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
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("CBNZ must complete its direct realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let placed = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(placed).unwrap();
    let staged =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert!(staged.source().source().text_section().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| instruction.byte_count == 0));
    assert_eq!(
        validate_optimized_object_artifact(&staged).unwrap(),
        staged.custody()
    );

    let mut wrong_magic = staged.artifact().encode();
    wrong_magic[0] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&wrong_magic),
        Err(OptimizedObjectArtifactRecordDecodeError::WrongMagic)
    );
    let mut wrong_version = staged.manifest().record().encode();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&wrong_version),
        Err(OptimizedObjectArtifactManifestDecodeError::UnsupportedVersion(2))
    );
    let mut trailing = staged.artifact().encode();
    trailing.push(0);
    assert_eq!(
        OptimizedObjectArtifactRecord::decode(&trailing),
        Err(OptimizedObjectArtifactRecordDecodeError::TrailingBytes)
    );
    let mut stale = staged.manifest().record().encode();
    stale[12] ^= 1;
    assert_eq!(
        OptimizedObjectArtifactManifest::decode(&stale),
        Err(OptimizedObjectArtifactManifestDecodeError::IdentityMismatch)
    );
}

#[test]
fn relocation_free_text_section_preserves_disconnected_function_order_without_padding() {
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
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct function-relative realization")
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    let mut fragments = emitted.fragments().clone();
    let entry = fragments.entry;
    let first_length = fragments.functions[0].byte_count;
    let mut detached = fragments.functions[0].clone();
    detached.machine = MachineId::new(1).unwrap();
    fragments.functions.push(detached);
    fragments.identity = fragments.recomputed_identity();
    let expected_machines = [entry, MachineId::new(1).unwrap()];
    let mut placed =
        crate::stages::artifacts::function_fragment_text_section::place_fragments_for_test(
            &fragments,
        )
        .unwrap();
    assert_eq!(
        placed
            .functions
            .iter()
            .map(|function| function.machine)
            .collect::<Vec<_>>(),
        expected_machines
    );
    assert_eq!(placed.functions[0].section_offset, 0);
    assert_eq!(placed.functions[1].section_offset, first_length);
    assert_eq!(placed.semantic_entry, expected_machines[0]);
    assert_eq!(placed.semantic_entry_offset, 0);
    assert_eq!(placed.byte_count, first_length * 2);
    assert_eq!(
        placed.bytes,
        [
            fragments.functions[0].bytes.as_slice(),
            fragments.functions[1].bytes.as_slice(),
        ]
        .concat()
    );

    let replay =
        crate::stages::artifacts::function_fragment_text_section::place_fragments_for_test(
            &fragments,
        )
        .unwrap();
    assert_eq!(replay, placed);
    placed.functions.swap(0, 1);
    placed.identity = placed.recomputed_identity();
    assert_ne!(placed, replay);
}

#[test]
fn relocation_free_fragment_emission_accepts_both_selected_lowering_compositions() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let x86_selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ])
    .unwrap();
    let x86_optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(x86_selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let x86_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        x86_optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = x86_physical
    else {
        panic!("combined x86 suite must retain selected-lowering custody")
    };
    let x86 = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(Box::new(
            realization,
        )),
    )
    .unwrap();
    assert_eq!(
        validate_optimized_function_fragment_emission(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_optimized_relocation_free_text_section(x86).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_optimized_relocation_free_object_container(x86).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&x86).unwrap(),
        x86.custody()
    );
    let x86 = stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), x86)
        .unwrap();
    assert_eq!(
        validate_optimized_object_artifact(&x86).unwrap(),
        x86.custody()
    );

    let arm_selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
    ])
    .unwrap();
    let arm_optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(arm_selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let arm_physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
        arm_optimized,
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        arm_physical
    else {
        panic!("combined AArch64 suite must retain both phase completions")
    };
    let arm = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        validate_optimized_function_fragment_emission(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_optimized_relocation_free_text_section(arm).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_text_section(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_optimized_relocation_free_object_container(arm).unwrap();
    assert_eq!(
        validate_optimized_relocation_free_object_container(&arm).unwrap(),
        arm.custody()
    );
    let arm = stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), arm)
        .unwrap();
    assert_eq!(
        validate_optimized_object_artifact(&arm).unwrap(),
        arm.custody()
    );
}

#[test]
fn rel8_fragment_emission_rejects_selected_lowering_without_the_named_layout_rule() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
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
    let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = physical else {
        panic!("selected lowering must retain its completed realization")
    };
    assert!(realization.relaxation().is_none());
    assert!(matches!(
        stage_optimized_function_fragment_emission(
            StagedOptimizedFunctionFragmentEmissionSource::X86Rel8AfterSelectedLowering(Box::new(
                realization
            ),),
        ),
        Err(FunctionFragmentEmissionError::MissingX86Rel8Realization)
    ));
}

#[test]
fn x86_rel8_selection_rejects_a_non_x86_target_without_a_realization() {
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
    assert!(matches!(
        stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        ),
        Err(
            OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization(
                FunctionRelativeOptimizationRealizationError::RuleCatalog(
                    FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                        optimization: Optimization::X86RelaxConditionalBranchesToRel8V1,
                        required: omega_target::Architecture::X86_64,
                        actual: omega_target::Architecture::Aarch64,
                    }
                )
            )
        )
    ));
}

#[test]
fn selected_lowering_suite_enforces_one_aggregate_budget() {
    let target = NativeTarget::linux_x64();
    let selections = OptimizationSelections::new([
        Optimization::CopyPropagation,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections.clone(),
                selected_lowering_budget(),
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let reference = run_selected_lowering_optimizations(source).unwrap();
    let attempt = reference.attempt();
    let component_usages = [
        attempt.choices().receipt().usage(),
        attempt.recovery().receipt().usage(),
        attempt.fold().receipt().usage(),
    ];
    let maximum = |field: fn(OptimizationWorkUsage) -> u64| {
        component_usages
            .into_iter()
            .map(field)
            .max()
            .unwrap()
            .max(1)
    };
    let component_only_budget = OptimizationWorkBudget::new(
        maximum(|usage| usage.rule_evaluations),
        maximum(|usage| usage.candidates),
        maximum(|usage| usage.validation_steps),
        maximum(|usage| usage.commits),
        maximum(|usage| usage.iterations),
    )
    .unwrap();
    assert!(component_usages
        .into_iter()
        .all(|usage| usage.within(component_only_budget)));

    let source = stage_optimized_allocation_legality(
        stage_optimized_live_ranges(
            stage_optimized_liveness(staged_exact_add_conditional_with_selections(
                target,
                selections,
                component_only_budget,
            ))
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        run_selected_lowering_optimizations(source),
        Err(OptimizedLiteralFoldCustodyError::SelectedLoweringBudgetExceeded { .. })
    ));
}
