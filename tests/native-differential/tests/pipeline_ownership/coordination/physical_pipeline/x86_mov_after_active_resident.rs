//! Exact active-resident recovery followed by x86 MOV-r32 realization.

use crate::FunctionFragmentReplayInputs;
use isa_x86_64::encode_x86_64_mov_r32_imm32_i64_materialization;

use crate::tests::*;

#[test]
fn active_resident_rematerialization_composes_with_mov_r32_through_publication() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
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
    let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
        optimized,
        NativeTarget::linux_x64(),
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
    let StagedOptimizedPostAllocationMachineOptimization::X86MovR32Imm32(materialization) =
        realization.optimization()
    else {
        panic!("the exact pair must retain the MOV-r32 optimization result")
    };

    let empty = OptimizationSelections::default().identity();
    let manifest = realization.manifest().record();
    let optimization_custody = realization.optimization().custody().unwrap();
    let rematerialization_plan = rematerialization.plan();
    let fresh_materialize = rematerialization_plan.functions[0]
        .action
        .as_ref()
        .unwrap()
        .fresh_materialize;
    let mov_plan = materialization.materialization().plan();
    assert_eq!(staged.selections(), selections.identity());
    assert_eq!(staged.selected_lowering_completion(), None);
    assert!(
        staged
            .allocation_recovery_function_relative_realization()
            .is_none()
    );
    assert_eq!(manifest.selections, selections.identity());
    assert_eq!(
        manifest.allocation_recovery_selections,
        OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap()
        .identity()
    );
    assert_eq!(manifest.selected_lowering_selections, empty);
    assert_eq!(manifest.function_relative_layout_selections, empty);
    assert_eq!(
        manifest.post_allocation_machine_selections,
        OptimizationSelections::new([
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        ])
        .unwrap()
        .identity()
    );
    assert_eq!(recovery_custody.applied_count(), 1);
    assert_eq!(recovery_custody.rewritten_use_count(), 2);
    assert_eq!(
        staged
            .post_allocation_manifest()
            .record()
            .selected_transformations,
        [
            PostAllocationSelectedTransformation::PressureRematerialization(
                rematerialization.receipt().identity(),
            )
        ]
    );
    assert!(
        mov_plan
            .actions
            .iter()
            .any(|action| action.instruction == fresh_materialize)
    );
    assert_eq!(optimization_custody.selections(), selections.identity());
    assert_eq!(
        optimization_custody.source(),
        realization.machine().machine().receipt().identity()
    );
    assert_eq!(
        manifest.post_allocation_machine_optimization,
        Some(optimization_custody)
    );
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization)
            .unwrap(),
        *realization.custody()
    );

    let physical = allocation.register_environment().physical();
    let expected_rows = mov_plan
        .actions
        .iter()
        .map(|action| {
            let canonical = encode_x86_64_mov_r32_imm32_i64_materialization(
                physical,
                action.destination.destination_view,
                IntegerValue::Unsigned(action.literal_bits.into()),
            )
            .unwrap();
            let baseline = realization
                .baseline_encoding()
                .rows()
                .iter()
                .find(|row| row.instruction == action.instruction)
                .unwrap();
            let selected = realization
                .encoding()
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
                panic!("the selected materialization must be encoded")
            };
            assert_eq!(baseline_bytes.len(), 10);
            assert_eq!(bytes, canonical.bytes());
            assert_eq!(bytes.len(), usize::from(action.selected_byte_count));
            assert_eq!(canonical.value_bits(), action.literal_bits);
            assert!(!canonical.footprint().writes_rflags);
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
    assert_eq!(
        emitted.manifest().record().source_kind,
        FunctionFragmentEmissionSourceKind::PostAllocationMachineOptimizationV1 {
            optimization: Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        }
    );
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
    validate_optimized_function_fragment_emission(&emitted).unwrap();
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
fn active_resident_mov_r32_pair_rejects_the_wrong_target() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
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
            NativeTarget::linux_arm64(),
            &[],
        ),
        Err(
            OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
                post_allocation_machine_to_post_allocation_machine::PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                    optimization:
                        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
                    required: target::Architecture::X86_64,
                    actual: target::Architecture::Aarch64,
                }
            )
        )
    ));
}

#[test]
fn active_resident_mov_r32_realization_rejects_manifest_and_exit_corruption() {
    let mut staged = staged_active_resident_mov_r32_pair();
    let realization = (staged)
        .post_allocation_machine_mut_for_test()
        .unwrap_or_else(|| unreachable!());
    realization
        .manifest_mut()
        .record_mut()
        .allocation_recovery_selections = OptimizationSelections::default().identity();
    assert_eq!(
        validate_post_allocation_machine_function_relative_realization_custody(realization),
        Err(FunctionRelativeOptimizationRealizationError::RootMismatch)
    );

    let mut staged = staged_active_resident_mov_r32_pair();
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

fn staged_active_resident_mov_r32_pair() -> StagedOptimizedVerifiedPhysicalPipeline {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    let selections = OptimizationSelections::new([
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
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
