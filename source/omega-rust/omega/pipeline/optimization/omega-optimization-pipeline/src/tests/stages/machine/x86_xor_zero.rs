use omega_isa_x86_64::encode_x86_64_xor_zero_i64_materialization;

use crate::tests::*;

#[test]
fn x86_xor_zero_reaches_direct_whole_function_exit_with_exact_custody() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let machine_fixture = conditional_immediate_machine(18_000, integer_type, [0, 1]);
    let module = conditional_immediate_module(machine_fixture.id, vec![machine_fixture]);
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&ProofBundle {
        evidence_producers: Vec::new(),
        evidence: Vec::new(),
    })
    .unwrap();
    let selections =
        OptimizationSelections::new([Optimization::X86SelectXorZeroI64MaterializationV1]).unwrap();
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
    let exit_contract =
        stage_whole_function_exit_contract_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            &selected_encoding,
            &optimization,
            &selected_layout,
        )
        .unwrap();

    let StagedOptimizedPostAllocationMachineOptimization::X86XorZero(materialization) =
        &optimization
    else {
        panic!("the exact x86 selection must produce the XOR-zero rule result")
    };
    let plan = materialization.materialization().plan();
    let optimized_instruction = plan.actions[0].instruction;
    let receipt = materialization.materialization().receipt();
    let custody = optimization.custody().unwrap();
    assert!(custody.action_count() > 0);
    assert_eq!(custody.action_count(), plan.actions.len());
    assert_eq!(
        custody.optimization(),
        Optimization::X86SelectXorZeroI64MaterializationV1
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
        u64::try_from(custody.action_count()).unwrap() * 10
    );
    assert_eq!(
        custody.selected_bytes(),
        u64::try_from(custody.action_count()).unwrap() * 3
    );
    assert_eq!(
        selected_encoding.post_allocation_machine_optimization(),
        Some(custody)
    );
    assert_eq!(
        selected_layout.post_allocation_machine_optimization(),
        Some(custody)
    );
    assert_eq!(
        exit_contract.contract().layout_custody,
        WholeFunctionExitLayoutCustody::PostAllocationMachineOptimizationV1 {
            optimization: custody.optimization(),
            artifact_identity: custody.artifact_identity(),
        }
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
            panic!("the selected XOR-zero materialization must be encoded")
        };
        let canonical =
            encode_x86_64_xor_zero_i64_materialization(physical, action.destination.view).unwrap();
        assert_eq!(baseline_bytes.len(), 10);
        assert_eq!(bytes, canonical.bytes());
        assert_eq!(bytes.len(), 3);
        assert_eq!(canonical.destination(), action.destination.view);
        assert_eq!(canonical.value_bits(), 0);
        assert_eq!(
            footprint.register_reads,
            canonical.footprint().register_reads
        );
        assert_eq!(
            footprint.register_writes,
            canonical.footprint().register_writes
        );
        assert_eq!(
            footprint.encoded.implicit_unit_clobbers,
            action.rflags_units
        );
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
        u64::try_from(custody.action_count()).unwrap() * 7
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
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization(
        selected_stage.selected(),
        &machine,
        physical,
        &selected_encoding,
        &optimization,
        &selected_layout,
        &exit_contract,
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
    assert_eq!(
        validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            Some(&optimization),
            &corrupted_encoding,
        ),
        Err(OptimizedSelectedFormEncodingError::ArtifactMismatch)
    );
    assert!(
        validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            &corrupted_encoding,
            Some(&optimization),
            &selected_layout,
        )
        .is_err()
    );
    assert!(
        validate_whole_function_exit_contract_with_post_allocation_machine_optimization(
            selected_stage.selected(),
            &machine,
            physical,
            &corrupted_encoding,
            &optimization,
            &selected_layout,
            &exit_contract,
        )
        .is_err()
    );

    let realization =
        stage_post_allocation_machine_function_relative_realization(homes, machine, optimization)
            .unwrap();
    assert_eq!(realization.baseline_encoding(), &baseline_encoding);
    assert_eq!(realization.encoding(), &selected_encoding);
    assert_eq!(realization.baseline_layout(), &baseline_layout);
    assert_eq!(realization.layout(), &selected_layout);
    assert_eq!(realization.exit_contract(), &exit_contract);
    assert_eq!(
        realization.custody().optimization().optimization(),
        Optimization::X86SelectXorZeroI64MaterializationV1
    );
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_optimization,
        Some(custody)
    );
    validate_post_allocation_machine_function_relative_realization_custody(&realization).unwrap();

    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    assert_eq!(
        emitted.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
        }
    );
    let xor_row = emitted
        .fragments()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .find(|row| row.instruction == optimized_instruction)
        .unwrap();
    assert_eq!(xor_row.bytes.len(), 3);
    let fragment_manifest = emitted.manifest().record().identity;

    let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
    assert_eq!(
        text.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectXorZeroI64MaterializationV1,
        }
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
