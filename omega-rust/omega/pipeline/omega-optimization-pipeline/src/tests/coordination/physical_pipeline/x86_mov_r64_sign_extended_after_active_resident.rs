//! Exact active-resident recovery followed by x86 MOV-r64-imm32 realization.

use omega_isa_x86_64::encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization;

use crate::tests::*;

#[test]
fn active_resident_composes_with_sign_extended_mov_through_publication() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
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
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = &staged
    else {
        panic!("the admitted pair must reach the generic post-allocation realization")
    };
    let allocation = realization.allocation().current();
    let omega_selected_instructions_to_register_homes::AllocationEvidence::ActiveResidentRematerialization(recovery_custody) = allocation.evidence() else {
        panic!("allocation must retain independently replayed rematerialization evidence")
    };
    let rematerialization = realization
        .allocation()
        .rematerialization_proof_for_test()
        .unwrap();
    let StagedOptimizedPostAllocationMachineOptimization::X86MovR64Imm32SignExtended(
        materialization,
    ) = realization.optimization()
    else {
        panic!("the exact pair must retain the sign-extended MOV optimization result")
    };

    let fresh_materialize = rematerialization.plan().functions[0]
        .action
        .as_ref()
        .unwrap()
        .fresh_materialize;
    let plan = materialization.materialization().plan();
    let custody = realization.optimization().custody().unwrap();
    assert!(
        plan.actions
            .iter()
            .any(|action| action.instruction == fresh_materialize)
    );
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(recovery_custody.applied_count(), 1);
    assert_eq!(recovery_custody.rewritten_use_count(), 2);
    assert_eq!(custody.selections(), selections.identity());
    assert_eq!(
        custody.optimization(),
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1
    );
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_optimization,
        Some(custody)
    );
    assert_eq!(
        realization
            .manifest()
            .record()
            .allocation_recovery_selections,
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap()
        .identity()
    );
    assert_eq!(
        realization
            .manifest()
            .record()
            .post_allocation_machine_selections,
        OptimizationSelections::new([
            Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
        ])
        .unwrap()
        .identity()
    );
    validate_post_allocation_machine_function_relative_realization_custody(realization).unwrap();

    let physical = allocation.register_environment().physical();
    let expected_rows = plan
        .actions
        .iter()
        .map(|action| {
            let canonical = encode_x86_64_mov_r64_imm32_sign_extended_i64_materialization(
                physical,
                action.destination.destination_view,
                IntegerValue::Unsigned(action.literal_bits.into()),
            )
            .unwrap();
            let selected = realization
                .encoding()
                .rows()
                .iter()
                .find(|row| row.instruction == action.instruction)
                .unwrap();
            let SelectedFormEncodingState::Encoded { bytes, footprint } = &selected.state else {
                panic!("the selected materialization must be encoded")
            };
            assert_eq!(bytes, canonical.bytes());
            assert_eq!(footprint.encoded, canonical.footprint().encoded);
            (action.instruction, canonical.bytes().to_vec())
        })
        .collect::<Vec<_>>();

    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } = staged
    else {
        unreachable!()
    };
    let emitted = stage_optimized_function_fragment_emission(
        StagedOptimizedFunctionFragmentEmissionSource::PostAllocationMachine(Box::new(realization)),
    )
    .unwrap();
    for (instruction, expected) in expected_rows {
        let emitted_row = emitted
            .fragments()
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .find(|row| row.instruction == instruction)
            .unwrap();
        assert_eq!(emitted_row.bytes, expected);
    }
    let text = stage_optimized_relocation_free_text_section(emitted).unwrap();
    let object = stage_optimized_relocation_free_object_container(text).unwrap();
    let artifact =
        stage_validated_optimized_object_artifact(canonical_artifact(&semantic, &proof), object)
            .unwrap();
    validate_optimized_object_artifact(&artifact).unwrap();
    let callable = stage_validated_optimized_ordinary_callable_entry(artifact).unwrap();
    validate_optimized_ordinary_callable_entry(&callable).unwrap();
}

#[test]
fn active_resident_sign_extended_mov_rejects_realization_corruption() {
    let mut staged = staged_pair();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        unreachable!()
    };
    realization
        .manifest_mut()
        .record_mut()
        .post_allocation_machine_optimization = None;
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );

    let mut staged = staged_pair();
    let StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization } =
        &mut staged
    else {
        unreachable!()
    };
    realization
        .exit_contract_mut()
        .contract_mut()
        .layout_custody = WholeFunctionExitLayoutCustody::BaselineNearLayoutV1;
    assert!(matches!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::ExitContract(
            _
        ))
    ));
}

fn staged_pair() -> StagedOptimizedVerifiedPhysicalPipeline {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::X86SelectMovR64Imm32SignExtendedI64MaterializationV1,
    ])
    .unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
        &[],
    )
    .unwrap()
}
