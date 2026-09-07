//! Straight-line calls use the common physical graph, not a fixture topology.

use crate::tests::*;

mod executable;
mod frames;
mod register_arity;
mod windows;

#[derive(Debug, Clone, Copy)]
enum Sequence {
    Single,
    EqualConstants,
    InterleavedCallees,
}

fn sequence_artifact(sequence: Sequence) -> (Vec<u8>, Vec<u8>) {
    scalar_call_unit_artifact_with(|module| {
        let operations = &mut module.machines[0].blocks[0].operations;
        match sequence {
            Sequence::Single => operations.truncate(3),
            Sequence::EqualConstants => {
                operations[1].kind = operations[0].kind.clone();
            }
            Sequence::InterleavedCallees => {
                // Exercise real MOVN shortening on Arm64; the other small
                // constant still exercises mov-r32 shortening on x64.
                operations[0].kind = OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(u128::from(u64::MAX)),
                };
                let left = ValueId::new(SCALAR_CALL_UNIT_LEFT).unwrap();
                let right = ValueId::new(SCALAR_CALL_UNIT_RIGHT).unwrap();
                let first = ValueId::new(SCALAR_CALL_UNIT_FIRST_RESULT).unwrap();
                let second = ValueId::new(SCALAR_CALL_UNIT_SECOND_RESULT).unwrap();
                let third = ValueId::new(SCALAR_CALL_UNIT_THIRD_RESULT).unwrap();
                let mut other = conditional_u64_integer_equal_parameters_machine(22_100, [9, 4]);
                other.blocks.truncate(1);
                other.blocks[0].operations.clear();
                other.blocks[0].terminator = Terminator::Return {
                    edge: EdgeId::new(22_116).unwrap(),
                    value: other.parameters[1].id,
                    cleanup_actions: Vec::new(),
                };
                let OperationKind::Call { arguments, .. } = &mut operations[2].kind else {
                    unreachable!()
                };
                *arguments = vec![left, left];
                // Constant materialization may occur between calls, not just
                // in an artificial prefix. The later call uses both an earlier
                // result and this newly defined value.
                operations.swap(1, 2);
                let OperationKind::Call {
                    arguments, callee, ..
                } = &mut operations[3].kind
                else {
                    unreachable!()
                };
                *arguments = vec![first, right];
                *callee = other.id;
                let OperationKind::Call { arguments, .. } = &mut operations[4].kind else {
                    unreachable!()
                };
                *arguments = vec![second, first];
                let mut fourth = operations[4].clone();
                fourth.id = OperationId::new(22_001).unwrap();
                let OperationResult::Scalar(result) = &mut fourth.result else {
                    unreachable!()
                };
                result.id = ValueId::new(22_002).unwrap();
                let OperationKind::Call {
                    arguments, callee, ..
                } = &mut fourth.kind
                else {
                    unreachable!()
                };
                *arguments = vec![third, right];
                *callee = other.id;
                operations.push(fourth);
                module.machines.push(other);
            }
        }
    })
}

#[test]
fn single_scalar_call_uses_shared_instruction_selection() {
    let (semantic, proof) = sequence_artifact(Sequence::Single);
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target =
        lower_optimized_to_target_operations(optimized, NativeTarget::linux_x64()).unwrap();
    stage_optimized_instruction_selection(target)
        .expect("a single call is an ordinary call sequence, not an unsupported topology");
}

#[test]
fn ordered_scalar_calls_reach_shared_object_publication_with_empty_and_selected_phases() {
    for sequence in [
        Sequence::Single,
        Sequence::EqualConstants,
        Sequence::InterleavedCallees,
    ] {
        let (semantic, proof) = sequence_artifact(sequence);
        let module = terminal_codec::decode_module(&semantic).unwrap();
        let expected_calls = module.machines[0].blocks[0]
            .operations
            .iter()
            .filter_map(|operation| match operation.kind {
                OperationKind::Call { callee, .. } => Some((operation.id, callee)),
                _ => None,
            })
            .collect::<Vec<_>>();
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            for selections in [
                OptimizationSelections::new([]).unwrap(),
                OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
                OptimizationSelections::new([match target.architecture {
                    target::Architecture::X86_64 => {
                        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
                    }
                    target::Architecture::Aarch64 => {
                        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
                    }
                }])
                .unwrap(),
            ] {
                let optimized = optimize_artifact_sections(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    compiler_baseline_request_v1(&selections),
                )
                .unwrap();
                let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
                    optimized,
                    target,
                    &[],
                )
                .unwrap_or_else(|error| panic!("{sequence:?}, {target:?}: {error:?}"));
                let emitted = stage_optimized_function_fragment_emission(
                    physical.into_function_fragment_emission_source(),
                )
                .unwrap();
                let caller = emitted
                    .fragments()
                    .functions
                    .iter()
                    .find(|function| function.machine == module.machines[0].id)
                    .unwrap();
                let actual_calls = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .filter_map(|instruction| {
                        instruction
                            .internal_machine_fixup
                            .as_ref()
                            .map(|fixup| (instruction.provenance.operations[0], fixup.callee))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual_calls, expected_calls);
                let applied = stage_function_fragment_frame_application(emitted).unwrap();
                assert_eq!(applied.receipt().framed_function_count(), 1);
                let text = stage_optimized_fixed_frame_text_section(applied).unwrap();
                assert_eq!(
                    text.text_section().resolved_internal_machine_calls.len(),
                    expected_calls.len()
                );
                let object = stage_optimized_relocation_free_object_container(text).unwrap();
                assert_eq!(
                    validate_optimized_relocation_free_object_container(&object).unwrap(),
                    object.custody()
                );
                let artifact = stage_validated_optimized_object_artifact(
                    canonical_artifact(&semantic, &proof),
                    object,
                )
                .unwrap();
                assert_eq!(
                    validate_optimized_object_artifact(&artifact).unwrap(),
                    artifact.custody()
                );
            }
        }
    }
}

#[test]
fn selected_materialization_preserves_normal_call_encoding_and_replay() {
    use post_allocation_machine_to_post_allocation_machine::stage_optimized_post_allocation_machine_optimization;
    use post_allocation_machine_to_selected_form_encoding::{
        stage_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization as encode,
        validate_optimized_layout_independent_selected_form_encoding_with_post_allocation_machine_optimization as validate,
    };

    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let physical_optimization = match target.architecture {
            target::Architecture::X86_64 => {
                Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1
            }
            target::Architecture::Aarch64 => {
                Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1
            }
        };
        let (semantic, proof) = sequence_artifact(Sequence::InterleavedCallees);
        let selections = OptimizationSelections::new([physical_optimization]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&selections),
        )
        .unwrap();
        let selected = stage_optimized_instruction_selection(
            lower_optimized_to_target_operations(optimized, target).unwrap(),
        )
        .unwrap();
        let homes = stage_optimized_register_homes(
            stage_optimized_allocation_legality(
                stage_optimized_live_ranges(stage_optimized_liveness(selected).unwrap()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
        let optimized =
            stage_optimized_post_allocation_machine_optimization(&homes, &machine).unwrap();
        let selected = homes
            .legality_stage()
            .live_range_stage()
            .liveness_stage()
            .selected_stage();
        let physical = selected.register_environment().physical();
        let baseline = encode(selected.selected(), &machine, physical, None).unwrap();
        let changed = encode(selected.selected(), &machine, physical, Some(&optimized)).unwrap();
        validate(
            selected.selected(),
            &machine,
            physical,
            Some(&optimized),
            &changed,
        )
        .unwrap();
        assert!(validate(selected.selected(), &machine, physical, None, &changed).is_err());
        let calls = |encoded: &StagedOptimizedSelectedFormEncoding| {
            encoded
                .rows()
                .iter()
                .filter(|row| {
                    matches!(
                        row.state,
                        SelectedFormEncodingState::UnresolvedInternalMachineCall { .. }
                    )
                })
                .map(|row| (row.instruction, row.state.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(calls(&changed).len(), 4);
        assert_eq!(calls(&changed), calls(&baseline));
    }
}
