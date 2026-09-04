use crate::tests::*;

#[test]
fn function_relative_only_rel8_suite_shrinks_and_replays_without_selected_lowering() {
    let target = NativeTarget::linux_x64();
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let mut staged =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(staged.selected_lowering_completion(), None);
    assert!(staged.function_relative_realization().is_none());
    assert!(
        optimization_pipeline_report(&staged)
            .function_relative()
            .is_some()
    );
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } =
        &mut staged
    else {
        panic!("the exact function-relative phase must use its direct realization route")
    };
    assert_eq!(
        validate_function_relative_layout_optimization_realization_custody(realization).unwrap(),
        *realization.custody()
    );
    assert_eq!(realization.relaxation().actions().len(), 1);
    assert_eq!(
        realization
            .baseline_layout()
            .functions()
            .iter()
            .map(|function| function.byte_count)
            .sum::<u64>()
            .checked_sub(
                realization
                    .layout()
                    .functions()
                    .iter()
                    .map(|function| function.byte_count)
                    .sum::<u64>()
            ),
        Some(4)
    );
    let relaxed_branch = realization
        .layout()
        .functions()
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.branch.is_some())
        .unwrap();
    assert_eq!(&relaxed_branch.bytes[..1], [0x75]);

    let manifest = realization.manifest().record();
    assert_eq!(
        manifest.selected_lowering_selections,
        OptimizationSelections::default().identity()
    );
    assert_eq!(manifest.selected_lowering_completion, None);
    assert_eq!(
        manifest.function_relative_layout_selections,
        selections.identity()
    );
    assert_eq!(
        manifest.baseline_resolved_layout,
        realization.baseline_layout().identity()
    );
    assert_eq!(manifest.resolved_layout, realization.layout().identity());
    assert_eq!(
        manifest.x86_branch_relaxation,
        Some(realization.relaxation().identity())
    );
    assert!(matches!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 {
            relaxation
        } if relaxation == realization.relaxation().identity()
    ));
    let original = realization.manifest().record().resolved_layout;
    realization.manifest_mut().record_mut().resolved_layout =
        realization.baseline_layout().identity();
    assert_eq!(
        validate_function_relative_layout_optimization_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );
    realization.manifest_mut().record_mut().resolved_layout = original;
    assert_eq!(
        validate_function_relative_layout_optimization_realization_custody(realization).unwrap(),
        *realization.custody()
    );
}

#[test]
fn selected_lowering_and_rel8_phases_retain_both_completion_receipts() {
    let target = NativeTarget::linux_x64();
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections = OptimizationSelections::new([
        Optimization::SelectedIncomingU12ExactAddImmediate,
        Optimization::X86RelaxConditionalBranchesToRel8V1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let staged =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::SelectedLowering { realization } = &staged else {
        panic!("the selected-lowering phase remains the owning physical route")
    };
    assert_eq!(
        validate_selected_lowering_function_relative_realization_custody(realization).unwrap(),
        *realization.custody()
    );
    let relaxation = realization
        .relaxation()
        .expect("the independently selected layout phase must execute");
    assert_eq!(relaxation.actions().len(), 1);
    let manifest = realization.manifest().record();
    assert_eq!(
        manifest.selected_lowering_completion,
        staged.selected_lowering_completion()
    );
    assert_eq!(
        manifest.function_relative_layout_selections,
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1,])
            .unwrap()
            .identity()
    );
    assert_eq!(manifest.x86_branch_relaxation, Some(relaxation.identity()));
    assert_eq!(manifest.resolved_layout, relaxation.layout().identity());
    assert_eq!(
        manifest.baseline_resolved_layout,
        realization.baseline_layout().identity()
    );
}

#[test]
fn relocation_free_rel8_fragment_emission_retains_bytes_fuel_and_manifest_custody() {
    let target = NativeTarget::linux_x64();
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
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::FunctionRelativeLayout { realization } = physical
    else {
        panic!("rel8 must complete its direct function-relative realization")
    };
    let mut emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::X86Rel8Direct(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted).unwrap(),
        emitted.custody()
    );
    let fragments = emitted.fragments();
    assert_eq!(fragments.functions.len(), 1);
    let function = &fragments.functions[0];
    let flattened = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .flat_map(|row| row.bytes.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(flattened, function.bytes);
    let branch = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.branch.is_some())
        .unwrap();
    assert_eq!(branch.bytes[0], 0x75);
    let omega_machine_code::FunctionFragmentControlProvenance::ConditionalBranch {
        when_taken,
        when_fallthrough,
        ..
    } = &branch.control
    else {
        panic!("resolved rel8 branch must retain both semantic successors")
    };
    assert!(!when_taken.fuel.is_empty());
    assert!(!when_fallthrough.fuel.is_empty());
    let record = emitted.manifest().record();
    assert_eq!(
        record.source_kind,
        FunctionFragmentEmissionSourceKind::X86Rel8V1
    );
    assert_eq!(record.fragments, fragments.identity);
    assert_eq!(record.statistics.zero_byte_instruction_spans, 0);
    assert!(record.statistics.logical_fuel_settlements > 0);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&record.encode()),
        Ok(record.clone())
    );
    let mut trailing = record.encode();
    trailing.push(0);
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&trailing),
        Err(FunctionFragmentEmissionManifestDecodeError::TrailingBytes)
    );
    let mut stale_identity = record.encode();
    stale_identity[12] ^= 1;
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&stale_identity),
        Err(FunctionFragmentEmissionManifestDecodeError::IdentityMismatch)
    );
    let mut wrong_version = record.encode();
    wrong_version[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&wrong_version),
        Err(FunctionFragmentEmissionManifestDecodeError::UnsupportedVersion(1))
    );
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&record.encode()[..20]),
        Err(FunctionFragmentEmissionManifestDecodeError::Truncated)
    );

    let original_control = emitted.fragments().functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|row| row.branch.is_some())
        .unwrap()
        .control
        .clone();
    let branch = emitted.fragments_mut().functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.branch.is_some())
        .unwrap();
    let omega_machine_code::FunctionFragmentControlProvenance::ConditionalBranch {
        when_taken, ..
    } = &mut branch.control
    else {
        unreachable!()
    };
    when_taken.fuel.clear();
    let fuel_corruption_identity = emitted.fragments().recomputed_identity();
    assert_ne!(fuel_corruption_identity, emitted.custody().fragments());
    emitted.fragments_mut().identity = fuel_corruption_identity;
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted),
        Err(FunctionFragmentEmissionError::ArtifactMismatch)
    );
    let branch = emitted.fragments_mut().functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.branch.is_some())
        .unwrap();
    branch.control = original_control;
    let restored_identity = emitted.fragments().recomputed_identity();
    emitted.fragments_mut().identity = restored_identity;
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted).unwrap(),
        emitted.custody()
    );

    let row = emitted.fragments_mut().functions[0].blocks[0]
        .instructions
        .iter_mut()
        .find(|row| !row.bytes.is_empty())
        .unwrap();
    row.bytes[0] ^= 1;
    let corrupted_identity = emitted.fragments().recomputed_identity();
    emitted.fragments_mut().identity = corrupted_identity;
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted),
        Err(FunctionFragmentEmissionError::ArtifactMismatch)
    );
}

#[test]
fn relocation_free_cbnz_fragment_emission_retains_the_elided_compare_span() {
    let target = NativeTarget::linux_arm64();
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
    let physical =
        stage_optimized_verified_physical_pipeline_with_provider_executions(optimized, target, &[])
            .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = physical
    else {
        panic!("CBNZ must complete its direct function-relative realization")
    };
    let mut emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    let function = &emitted.fragments().functions[0];
    let rows = function
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
    let branch = rows.iter().find(|row| row.branch.is_some()).unwrap();
    assert!(compare.bytes.is_empty());
    assert!(compare.provenance.fuel.is_empty());
    assert_eq!(compare.offset, branch.offset);
    assert_eq!(branch.bytes.len(), 4);
    assert_eq!(
        u32::from_le_bytes(branch.bytes.as_slice().try_into().unwrap()) & 0xff00_0000,
        0xb500_0000
    );
    assert_eq!(
        emitted
            .manifest()
            .record()
            .statistics
            .zero_byte_instruction_spans,
        1
    );
    assert_eq!(
        emitted.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        }
    );
    let encoded = emitted.manifest().record().encode();
    assert_eq!(&encoded[8..12], &10_u32.to_le_bytes());
    assert_eq!(encoded[45], 2);
    assert_eq!(
        encoded[46],
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1 as u8
    );
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&encoded).unwrap(),
        *emitted.manifest().record()
    );
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted).unwrap(),
        emitted.custody()
    );

    let block = emitted.fragments_mut().functions[0]
        .blocks
        .iter_mut()
        .find(|block| block.instructions.iter().any(|row| row.bytes.is_empty()))
        .unwrap();
    block.instructions.retain(|row| !row.bytes.is_empty());
    let corrupted_identity = emitted.fragments().recomputed_identity();
    emitted.fragments_mut().identity = corrupted_identity;
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted),
        Err(FunctionFragmentEmissionError::ArtifactMismatch)
    );
}

#[test]
fn aarch64_movn_reaches_fragments_text_object_artifact_and_callable_for_both_routes() {
    use omega_calling_conventions::{CallingPolicy, MachineRegister};

    for (target, selected_lowering) in [
        (NativeTarget::linux_arm64(), false),
        (NativeTarget::macos_arm64(), true),
    ] {
        std::thread::Builder::new()
            .name("aarch64-movn-object-custody".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let (semantic, proof) =
                    conditional_active_resident_exact_add_chain_artifact_with_false_literal(
                        IntegerValue::Unsigned(u64::MAX as u128),
                    );
                let selections = if selected_lowering {
                    OptimizationSelections::new([
                        Optimization::SelectedIncomingU12ExactAddImmediate,
                        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    ])
                    .unwrap()
                } else {
                    OptimizationSelections::new([
                        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    ])
                    .unwrap()
                };
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    ExplicitOptimizationRequest::new(
                        selections.clone(),
                        selected_lowering_budget(),
                    )
                    .unwrap(),
                )
                .unwrap();
                let physical =
                    stage_optimized_verified_physical_pipeline_with_provider_executions(
                        optimized,
                        target,
                        &[],
                    )
                    .unwrap();
                let source = match physical {
                    StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine {
                        realization,
                    } => StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(
                        Box::new(realization),
                    ),
                    _ => panic!("MOVN fixture must retain the corresponding realization route"),
                };

                let mut emitted = stage_optimized_function_fragment_emission(source).unwrap();
                assert_eq!(
                    validate_optimized_function_fragment_emission(&emitted).unwrap(),
                    emitted.custody()
                );
                assert_eq!(
                    emitted.manifest().record().source_kind,
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization:
                            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    }
                );
                assert_eq!(
                    emitted.manifest().record().selections,
                    selections.identity()
                );

                let realization = match emitted.source() {
                    StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(
                        realization,
                    ) => realization,
                    _ => unreachable!(),
                };
                let materialization = match realization.optimization() {
                    StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(
                        materialization,
                    ) => materialization,
                    _ => unreachable!(),
                };
                let baseline_layout = realization.baseline_layout();
                let final_layout = realization.layout();
                let exit = realization.exit_contract();
                let realization_manifest = realization.manifest();
                let action = &materialization.materialization().plan().actions[0];
                let action_instruction = action.instruction;
                let exit_identity = exit.identity();
                let baseline_row = baseline_layout
                    .functions()
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap();
                let final_row = final_layout
                    .functions()
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap();
                let fragment_row = emitted
                    .fragments()
                    .functions
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap();
                assert_eq!(fragment_row.bytes, final_row.bytes);
                assert_eq!(
                    fragment_row.bytes.len(),
                    usize::from(action.recipe.word_count().unwrap()) * 4
                );
                assert!(fragment_row.bytes.len() < baseline_row.bytes.len());
                assert_eq!(
                    emitted.manifest().record().source_realization,
                    realization_manifest.record().identity
                );
                assert_eq!(
                    emitted.manifest().record().whole_function_exit_contract,
                    exit_identity
                );
                assert_eq!(
                    emitted.manifest().record().final_pre_layout,
                    final_layout.pre_layout()
                );
                assert_eq!(
                    emitted.manifest().record().final_resolved_layout,
                    final_layout.identity()
                );
                let receipt = materialization.custody();
                let baseline_bytes = baseline_layout
                    .functions()
                    .iter()
                    .map(|function| function.byte_count)
                    .sum::<u64>();
                let fragment_bytes = emitted
                    .fragments()
                    .functions
                    .iter()
                    .map(|function| function.byte_count)
                    .sum::<u64>();
                assert_eq!(
                    baseline_bytes.checked_sub(fragment_bytes),
                    receipt
                        .baseline_words()
                        .checked_sub(receipt.selected_words())
                        .and_then(|words| words.checked_mul(4))
                );
                let fragment_manifest = emitted.manifest().record().clone();
                let fragment_encoded = fragment_manifest.encode();
                assert_eq!(&fragment_encoded[8..12], &10_u32.to_le_bytes());
                assert_eq!(fragment_encoded[45], 2);
                assert_eq!(
                    fragment_encoded[46],
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 as u8
                );
                assert_eq!(
                    FunctionFragmentEmissionManifest::decode(&fragment_encoded),
                    Ok(fragment_manifest.clone())
                );
                let mut xor_fragment_manifest = fragment_manifest.clone();
                xor_fragment_manifest.source_kind =
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                    };
                xor_fragment_manifest.identity = xor_fragment_manifest.recomputed_identity();
                let xor_fragment_encoded = xor_fragment_manifest.encode();
                assert_eq!(xor_fragment_encoded[45], 2);
                assert_eq!(
                    xor_fragment_encoded[46],
                    Optimization::X86SelectXorZeroI64MaterializationV1 as u8
                );
                assert_eq!(
                    FunctionFragmentEmissionManifest::decode(&xor_fragment_encoded),
                    Ok(xor_fragment_manifest)
                );
                let mut unknown_fragment_optimization = fragment_encoded.clone();
                unknown_fragment_optimization[46] = u8::MAX;
                assert_eq!(
                    FunctionFragmentEmissionManifest::decode(&unknown_fragment_optimization),
                    Err(
                        FunctionFragmentEmissionManifestDecodeError::UnknownPostAllocationMachineOptimization(
                            u8::MAX,
                        ),
                    )
                );
                let mut unknown_fragment_source = fragment_encoded;
                unknown_fragment_source[45] = 8;
                assert_eq!(
                    FunctionFragmentEmissionManifest::decode(&unknown_fragment_source),
                    Err(FunctionFragmentEmissionManifestDecodeError::UnknownSourceKind(8))
                );
                let expected_text_bytes = emitted
                    .fragments()
                    .functions
                    .iter()
                    .flat_map(|function| function.bytes.iter().copied())
                    .collect::<Vec<_>>();
                let original_fragment_identity = emitted.fragments().identity;
                let original_movn_byte = emitted
                    .fragments()
                    .functions
                    .iter()
                    .flat_map(|function| &function.blocks)
                    .flat_map(|block| &block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap()
                    .bytes[0];
                emitted
                    .fragments_mut()
                    .functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap()
                    .bytes[0] ^= 1;
                let corrupted_fragment_identity = emitted.fragments().recomputed_identity();
                emitted.fragments_mut().identity = corrupted_fragment_identity;
                assert_eq!(
                    validate_optimized_function_fragment_emission(&emitted),
                    Err(FunctionFragmentEmissionError::ArtifactMismatch)
                );
                emitted
                    .fragments_mut()
                    .functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.instruction == action_instruction)
                    .unwrap()
                    .bytes[0] = original_movn_byte;
                let restored_fragment_identity = emitted.fragments().recomputed_identity();
                assert_eq!(restored_fragment_identity, original_fragment_identity);
                emitted.fragments_mut().identity = restored_fragment_identity;
                assert_eq!(
                    validate_optimized_function_fragment_emission(&emitted).unwrap(),
                    emitted.custody()
                );

                let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
                assert_eq!(
                    validate_optimized_relocation_free_text_section(&text).unwrap(),
                    text.custody()
                );
                assert_eq!(text.text_section().section_alignment, 4);
                assert_eq!(text.text_section().bytes, expected_text_bytes);
                assert_eq!(
                    text.text_section().relocation_requirements,
                    omega_object_file::TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
                );
                assert_eq!(
                    text.manifest().record().source_kind,
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization:
                            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    }
                );
                assert_eq!(text.manifest().record().statistics.padding_bytes, 0);
                assert_eq!(text.manifest().record().statistics.relocation_requirements, 0);
                let text_manifest = text.manifest().record().clone();
                let text_encoded = text_manifest.encode();
                assert_eq!(&text_encoded[8..12], &11_u32.to_le_bytes());
                assert_eq!(text_encoded[45], 1);
                assert_eq!(text_encoded[46], 2);
                assert_eq!(
                    text_encoded[47],
                    Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1 as u8
                );
                assert_eq!(
                    FunctionFragmentTextSectionManifest::decode(&text_encoded),
                    Ok(text_manifest.clone())
                );
                let mut xor_text_manifest = text_manifest.clone();
                xor_text_manifest.source_kind =
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
                    };
                xor_text_manifest.identity = xor_text_manifest.recomputed_identity();
                let xor_text_encoded = xor_text_manifest.encode();
                assert_eq!(xor_text_encoded[45], 1);
                assert_eq!(xor_text_encoded[46], 2);
                assert_eq!(
                    xor_text_encoded[47],
                    Optimization::X86SelectXorZeroI64MaterializationV1 as u8
                );
                assert_eq!(
                    FunctionFragmentTextSectionManifest::decode(&xor_text_encoded),
                    Ok(xor_text_manifest)
                );
                let mut unknown_text_optimization = text_encoded.clone();
                unknown_text_optimization[47] = u8::MAX;
                assert_eq!(
                    FunctionFragmentTextSectionManifest::decode(&unknown_text_optimization),
                    Err(
                        FunctionFragmentTextSectionManifestDecodeError::UnknownPostAllocationMachineOptimization(
                            u8::MAX,
                        ),
                    )
                );
                let mut unknown_text_source = text_encoded;
                unknown_text_source[46] = 8;
                assert_eq!(
                    FunctionFragmentTextSectionManifest::decode(&unknown_text_source),
                    Err(FunctionFragmentTextSectionManifestDecodeError::UnknownSourceKind(8))
                );

                let object = stage_optimized_relocation_free_object_container(text).unwrap();
                assert_eq!(
                    validate_optimized_relocation_free_object_container(&object).unwrap(),
                    object.custody()
                );
                assert_eq!(object.object().text_section.bytes, expected_text_bytes);
                assert_eq!(object.object().relocation_record_count, 0);
                assert_eq!(object.object().symbols.len(), 1);
                assert_eq!(
                    object.object().symbols[0].linkage,
                    omega_object_file::RelocationFreeObjectSymbolLinkage::ObjectLocalV1
                );
                assert_eq!(
                    object.object().symbols[0].role,
                    omega_object_file::RelocationFreeObjectSymbolRole::SemanticEntryV1
                );
                let object_manifest = object.manifest().record().identity;
                let artifact = stage_validated_optimized_object_artifact(
                    canonical_artifact(&semantic, &proof),
                    object,
                )
                .unwrap();
                assert_eq!(
                    validate_optimized_object_artifact(&artifact).unwrap(),
                    artifact.custody()
                );
                assert_eq!(artifact.artifact().selections, selections.identity());
                assert_eq!(
                    artifact.artifact().function_fragment_manifest,
                    fragment_manifest.identity
                );
                assert_eq!(artifact.artifact().text_section_manifest, text_manifest.identity);
                assert_eq!(artifact.artifact().object_container_manifest, object_manifest);
                assert_eq!(
                    OptimizedObjectArtifactRecord::decode(&artifact.artifact().encode()).unwrap(),
                    *artifact.artifact()
                );
                let artifact_report = optimization_pipeline_report_from_object_artifact(&artifact);
                assert_eq!(
                    artifact_report.function_fragment().unwrap().source_kind,
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization:
                            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    }
                );
                assert!(
                    artifact_report
                        .function_relative()
                        .unwrap()
                        .post_allocation_machine_optimization
                        .is_some()
                );
                assert!(artifact_report.ordinary_callable_entry().is_none());

                let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
                    .expect("the MOVN semantic entry remains an ordinary scalar callable");
                assert_eq!(
                    validate_optimized_ordinary_callable_entry(&callable).unwrap(),
                    callable.custody()
                );
                assert_eq!(callable.entry().calling_policy, CallingPolicy::Aapcs64);
                assert_eq!(
                    callable.entry().parameters[0].abi_register,
                    MachineRegister::Aarch64X(0)
                );
                assert_eq!(
                    callable.entry().result.abi_register,
                    MachineRegister::Aarch64X(0)
                );
                assert_eq!(callable.entry().returns.len(), 2);
                assert_eq!(callable.entry().exit_contract, exit_identity);
                assert_eq!(
                    callable.entry().disposition,
                    OptimizedOrdinaryCallableEntryDisposition::ExternalProcessEntryBridgeRequiredV1
                );
                let report = optimization_pipeline_report_from_ordinary_callable_entry(&callable);
                assert_eq!(
                    report.function_fragment().unwrap().source_kind,
                    FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
                        optimization:
                            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
                    }
                );
                assert_eq!(
                    report.ordinary_callable_entry().unwrap().entry,
                    callable.entry().identity
                );
                let human = report
                    .render_human_text(OptimizationReportRequest::EmitHumanText)
                    .unwrap();
                assert!(human.contains("external process entry bridge: required"));
                assert!(human.contains("publication: unavailable"));
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
