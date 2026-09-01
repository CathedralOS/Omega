//! Direct publication and composition canaries with named custody corruption.

use omega_isa_x86_64::encode_x86_64_mov_r32_imm32_i64_materialization;

use crate::tests::*;

mod custody_corruption;

#[test]
fn x86_mov_r32_imm32_reaches_realization_with_replayable_zero_extension_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture = conditional_immediate_machine(18_200, integer_type, [1, u32::MAX.into()]);
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    let selected = stage_optimized_instruction_selection(target).unwrap();
    let liveness = stage_optimized_liveness(selected).unwrap();
    let ranges = stage_optimized_live_ranges(liveness).unwrap();
    let legality = stage_optimized_allocation_legality(ranges).unwrap();
    let homes = stage_optimized_register_homes(legality).unwrap();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let optimization =
        stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let physical = selected_stage.register_environment().physical();
    let baseline_encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected_stage.selected(),
        &machine,
        physical,
    )
    .unwrap();
    let selected_encoding =
        stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            Some(&optimization),
        )
        .unwrap();
    let baseline_layout = stage_optimized_resolved_selected_form_layout(
        selected_stage.selected(),
        &machine,
        physical,
        &baseline_encoding,
    )
    .unwrap();
    let selected_layout =
        stage_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            &selected_encoding,
            Some(&optimization),
        )
        .unwrap();

    let StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(materialization) =
        &optimization
    else {
        panic!("the exact x86 selection must produce the MOV-r32-imm32 rule result")
    };
    let plan = materialization.materialization().plan();
    let receipt = materialization.materialization().receipt();
    let custody = optimization.custody().unwrap();
    assert!(custody.action_count() > 0);
    assert_eq!(custody.action_count(), plan.actions.len());
    assert_eq!(
        custody.optimization(),
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
    );
    assert_eq!(custody.artifact_identity(), receipt.identity().bytes());
    assert_eq!(custody.selections(), selections.identity());
    assert_eq!(
        custody.post_allocation_machine_selections(),
        selections.identity()
    );
    assert_eq!(custody.source(), machine.machine().receipt().identity());
    assert_eq!(
        custody.baseline_bytes(),
        plan.actions
            .iter()
            .map(|action| u64::from(action.baseline_byte_count))
            .sum::<u64>()
    );
    assert_eq!(
        custody.selected_bytes(),
        plan.actions
            .iter()
            .map(|action| u64::from(action.selected_byte_count))
            .sum::<u64>()
    );
    assert_eq!(
        selected_encoding.post_allocation_machine_optimization(),
        Some(custody)
    );
    assert_eq!(
        selected_layout.post_allocation_machine_optimization(),
        Some(custody)
    );

    for action in &plan.actions {
        let baseline_row = baseline_encoding
            .rows()
            .iter()
            .find(|row| row.instruction == action.instruction)
            .unwrap();
        let selected_row = selected_encoding
            .rows()
            .iter()
            .find(|row| row.instruction == action.instruction)
            .unwrap();
        let SelectedFormEncodingState::Encoded {
            bytes: baseline_bytes,
            ..
        } = &baseline_row.state
        else {
            panic!("the baseline materialization must be encoded")
        };
        let SelectedFormEncodingState::Encoded { bytes, footprint } = &selected_row.state else {
            panic!("the selected MOV-r32-imm32 materialization must be encoded")
        };
        let canonical = encode_x86_64_mov_r32_imm32_i64_materialization(
            physical,
            action.destination.destination_view,
            IntegerValue::Unsigned(action.literal_bits.into()),
        )
        .unwrap();
        assert_eq!(
            baseline_bytes.len(),
            usize::from(action.baseline_byte_count)
        );
        assert_eq!(bytes, canonical.bytes());
        assert_eq!(bytes.len(), usize::from(action.selected_byte_count));
        assert_eq!(canonical.destination(), action.destination.destination_view);
        assert_eq!(
            canonical.encoded_write_view(),
            action.destination.encoded_view
        );
        assert_eq!(canonical.value_bits(), action.literal_bits);
        assert_eq!(
            canonical.footprint().encoded_write_view_units,
            action.destination.encoded_storage_units
        );
        assert_eq!(
            canonical.footprint().encoded_write_units,
            action.destination.encoded_write_units
        );
        assert!(!canonical.footprint().writes_rflags);
        assert_eq!(footprint.encoded, canonical.footprint().encoded);
    }

    let baseline_bytes = baseline_layout
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    let selected_bytes = selected_layout
        .functions()
        .iter()
        .map(|function| function.byte_count)
        .sum::<u64>();
    assert_eq!(
        baseline_bytes - selected_bytes,
        custody.baseline_bytes() - custody.selected_bytes()
    );
    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected_stage.selected(),
        &machine,
        physical,
        Some(&optimization),
        &selected_encoding,
    )
    .unwrap();
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected_stage.selected(),
        &machine,
        physical,
        &selected_encoding,
        Some(&optimization),
        &selected_layout,
    )
    .unwrap();

    let mut corrupted_encoding = selected_encoding.clone();
    let corrupted_row = corrupted_encoding
        .rows_mut()
        .iter_mut()
        .find(|row| row.instruction == plan.actions[0].instruction)
        .unwrap();
    let SelectedFormEncodingState::Encoded { bytes, .. } = &mut corrupted_row.state else {
        unreachable!()
    };
    bytes[0] ^= 1;
    assert!(matches!(
        validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            Some(&optimization),
            &corrupted_encoding,
        ),
        Err(OptimizedSelectedFormEncodingError::X86_64MovR32Imm32(_))
    ));

    let expected_rows = plan
        .actions
        .iter()
        .map(|action| {
            let canonical = encode_x86_64_mov_r32_imm32_i64_materialization(
                physical,
                action.destination.destination_view,
                IntegerValue::Unsigned(action.literal_bits.into()),
            )
            .unwrap();
            (action.instruction, canonical.bytes().to_vec())
        })
        .collect::<Vec<_>>();
    let realization =
        stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
            .unwrap();
    assert_eq!(realization.baseline_encoding(), &baseline_encoding);
    assert_eq!(realization.encoding(), &selected_encoding);
    assert_eq!(realization.baseline_layout(), &baseline_layout);
    assert_eq!(realization.layout(), &selected_layout);
    assert_eq!(
        realization.custody().optimization().optimization(),
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
    );
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();
    let manifest = realization.manifest().record();
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(&manifest.encode()).unwrap(),
        *manifest
    );
    assert_eq!(manifest.post_allocation_machine_optimization, Some(custody));
    assert_eq!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            artifact_identity: custody.artifact_identity(),
        }
    );

    let mut emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        emitted.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        }
    );
    assert_eq!(
        FunctionFragmentEmissionManifest::decode(&emitted.manifest().record().encode()).unwrap(),
        *emitted.manifest().record()
    );
    for (instruction, expected) in &expected_rows {
        let row = emitted
            .fragments()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == *instruction)
            .unwrap();
        assert_eq!(&row.bytes, expected);
    }
    assert_eq!(
        validate_optimized_function_fragment_emission(&emitted).unwrap(),
        emitted.custody()
    );
    let first_instruction = expected_rows[0].0;
    emitted
        .fragments_mut()
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find(|row| row.instruction == first_instruction)
        .unwrap()
        .bytes[0] ^= 1;
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
        .find(|row| row.instruction == first_instruction)
        .unwrap()
        .bytes[0] ^= 1;
    validate_optimized_function_fragment_emission(&emitted).unwrap();
    let fragment_manifest = emitted.manifest().record().identity;

    let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
    assert_eq!(
        text.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        }
    );
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text.manifest().record().encode()).unwrap(),
        *text.manifest().record()
    );
    let text_manifest = text.manifest().record().identity;
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let object_manifest = object.manifest().record().identity;
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    assert_eq!(
        artifact.artifact().function_fragment_manifest,
        fragment_manifest
    );
    assert_eq!(artifact.artifact().text_section_manifest, text_manifest);
    assert_eq!(
        artifact.artifact().object_container_manifest,
        object_manifest
    );
    validate_optimized_object_artifact(&artifact).unwrap();
    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    validate_optimized_ordinary_callable_entry(&callable).unwrap();
}

#[test]
fn x86_mov_r32_imm32_and_xor_zero_reject_without_hidden_rule_ordering() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine = conditional_immediate_machine(18_300, integer_type, [0, 1]);
    let module = conditional_immediate_module(machine.id, vec![machine]);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        Optimization::X86SelectXorZeroI64MaterializationV1,
    ])
    .unwrap();
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
            NativeTarget::linux_x64(),
            &[],
        ),
        Err(
            OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
                omega_machine_optimizer::PostAllocationMachineRuleCatalogError::UnsupportedComposition(
                    _
                )
            )
        )
    ));
}
