//! Direct publication canary plus the exact rule's named custody-corruption rung.

use crate::FunctionFragmentReplayInputs;
use isa_x86_64::encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization;

use crate::tests::*;

mod custody_corruption;

#[test]
fn x86_mov_r64_imm32_sign_extended_reaches_publication_with_replayable_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture = conditional_immediate_machine(
        18_400,
        integer_type,
        [u128::from(i32::MAX as u32), u128::from(u64::MAX)],
    );
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof = terminal_codec::encode_proof_bundle(&ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
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

    let StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended(
        materialization,
    ) = &optimization
    else {
        panic!("the exact x86 selection must produce the MOV-r64-imm32 rule result")
    };
    let plan = materialization.materialization().plan();
    let receipt = materialization.materialization().receipt();
    let custody = optimization.custody().unwrap();
    assert_eq!(plan.actions.len(), 2);
    assert_eq!(custody.action_count(), plan.actions.len());
    assert_eq!(
        custody.optimization(),
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1
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

    let expected_rows = plan
        .actions
        .iter()
        .map(|action| {
            let baseline = baseline_encoding
                .rows()
                .iter()
                .find(|row| row.instruction == action.instruction)
                .unwrap();
            let selected = selected_encoding
                .rows()
                .iter()
                .find(|row| row.instruction == action.instruction)
                .unwrap();
            let SelectedFormEncodingState::Encoded {
                bytes: baseline_bytes,
                ..
            } = &baseline.state
            else {
                panic!("the baseline materialization must be encoded")
            };
            let SelectedFormEncodingState::Encoded { bytes, footprint } = &selected.state else {
                panic!("the selected sign-extended materialization must be encoded")
            };
            let canonical = encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
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
            assert!(!canonical.footprint().writes_rflags);
            assert_eq!(footprint.encoded, canonical.footprint().encoded);
            (action.instruction, canonical.bytes().to_vec())
        })
        .collect::<Vec<_>>();

    validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
        selected_stage.selected(),
        &machine,
        physical,
        Some(&optimization),
        &selected_encoding,
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
        Err(OptimizedSelectedFormEncodingError::X86_64MovR64Imm32SignExtended(_))
    ));

    let realization =
        stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
            .unwrap();
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_optimization,
        Some(custody)
    );
    assert_eq!(
        FunctionRelativeOptimizationRealizationManifest::decode(
            &realization.manifest().record().encode()
        )
        .unwrap(),
        *realization.manifest().record()
    );
    assert_eq!(
        realization.exit_contract().contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
            artifact_identity: custody.artifact_identity(),
        }
    );

    let mut emitted = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
    )
    .unwrap();
    assert_eq!(
        emitted.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
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
    validate_optimized_function_fragment_emission(&emitted).unwrap();
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

    let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
    assert_eq!(
        FunctionFragmentTextSectionManifest::decode(&text.manifest().record().encode()).unwrap(),
        *text.manifest().record()
    );
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    validate_optimized_object_artifact(&artifact).unwrap();
    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    validate_optimized_ordinary_callable_entry(&callable).unwrap();
}
