//! Exact active-resident recovery followed by AArch64 MOVN realization.

use crate::FunctionFragmentReplayInputs;
use isa_aarch64::encode_aarch64_shortest_movn_materialization;

use crate::tests::*;

const MOVN_RESIDENT_BITS: u64 = 0x7fff_ffff_ffff_0000;

#[test]
fn active_resident_composes_with_movn_through_publication() {
    let (semantic, proof) =
        conditional_active_resident_exact_add_chain_artifact_with_resident_literal(
            IntegerValue::Unsigned(u128::from(MOVN_RESIDENT_BITS)),
        );
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
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
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap();
    let realization = (staged)
        .post_allocation_machine_for_test()
        .unwrap_or_else(|| {
            panic!("the admitted pair must reach the generic post-allocation realization")
        });
    let allocation = realization.allocation().current();
    let selected_instructions_to_register_homes::AllocationEvidence::ActiveResidentRematerialization(recovery_custody) = allocation.evidence() else {
        panic!("allocation must retain independently replayed rematerialization evidence")
    };
    let rematerialization = realization
        .allocation()
        .rematerialization_proof_for_test()
        .unwrap();
    let StagedOptimizedPostAllocationMachineOptimization::Aarch64Movn(materialization) =
        realization.optimization()
    else {
        panic!("the exact pair must retain the MOVN optimization result")
    };

    let fresh_materialize = rematerialization.plan().functions[0]
        .action
        .as_ref()
        .unwrap()
        .fresh_materialize;
    let plan = materialization.materialization().plan();
    let action = plan
        .actions
        .iter()
        .find(|action| action.instruction == fresh_materialize)
        .expect("the machine rule must select the recovery-introduced materialization");
    let custody = realization.optimization().custody().unwrap();
    assert_eq!(action.literal_bits, MOVN_RESIDENT_BITS);
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(recovery_custody.applied_count(), 1);
    assert_eq!(recovery_custody.rewritten_use_count(), 2);
    assert_eq!(custody.selections(), selections.identity());
    assert_eq!(
        custody.optimization(),
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
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
            Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
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
            let canonical = encode_aarch64_shortest_movn_materialization(
                physical,
                action.destination.view,
                IntegerValue::Unsigned(u128::from(action.literal_bits)),
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

    let realization = (staged)
        .into_post_allocation_machine_for_test()
        .unwrap_or_else(|| unreachable!());
    let emitted = stage_optimized_function_fragment_emission(
        FunctionFragmentReplayInputs::PostAllocationMachine(Box::new(realization)).into(),
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
fn active_resident_movn_rejects_realization_corruption() {
    let mut staged = staged_pair();
    let realization = (staged)
        .post_allocation_machine_mut_for_test()
        .unwrap_or_else(|| unreachable!());
    realization
        .manifest_mut()
        .record_mut()
        .post_allocation_machine_optimization = None;
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );

    let mut staged = staged_pair();
    let realization = (staged)
        .post_allocation_machine_mut_for_test()
        .unwrap_or_else(|| unreachable!());
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
    let (semantic, proof) =
        conditional_active_resident_exact_add_chain_artifact_with_resident_literal(
            IntegerValue::Unsigned(u128::from(MOVN_RESIDENT_BITS)),
        );
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
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
        NativeTarget::linux_arm64(),
        &[],
    )
    .unwrap()
}
