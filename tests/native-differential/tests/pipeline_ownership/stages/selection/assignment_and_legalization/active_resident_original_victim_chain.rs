//! Public custody for the exact guarded-original prerequisite graph.

use crate::tests::*;

#[test]
fn original_victim_graph_legalization_retains_dependencies_and_is_independently_replayed() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) =
            conditional_active_resident_exact_add_original_victim_chain_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(
                OptimizationSelections::new([
                    Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                ])
                .unwrap(),
            ),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();
        let legalized = legalize_target_operations(
            target.target_operations(),
            target.optimized().plan(),
            target.optimized().unit(),
        )
        .unwrap();
        let function = legalized.plan().functions[0].conditional();
        assert_eq!(
            function.recipe,
            LegalizationRecipe::ReturnU64ExactIntegerSequenceConditionalV1,
        );
        let LegalizedLeafValue::ExactIntegerSequence(sequence) = &function.when_true.value else {
            panic!("exact graph uses the ordinary ordered sequence")
        };
        assert_eq!(sequence.steps.len(), 8);
        let legalized_operations::LegalizedIntegerStep::ExactBinary(join) = &sequence.steps[6]
        else {
            panic!("the join is the seventh source definition")
        };
        let legalized_operations::LegalizedIntegerStep::ExactBinary(result) = &sequence.steps[7]
        else {
            panic!("the result follows the join")
        };
        assert_eq!(result.right, join.source_value);
        assert_ne!(join.source_value, result.source_value);

        let mut corrupted = legalized.plan().clone();
        let LegalizedLeafValue::ExactIntegerSequence(sequence) =
            &mut corrupted.functions[0].conditional_mut().when_true.value
        else {
            unreachable!()
        };
        sequence.steps.swap(5, 6);
        assert!(
            validate_legalized_operations(
                target.target_operations(),
                target.optimized().plan(),
                target.optimized().unit(),
                corrupted,
            )
            .is_err()
        );
    }
}

#[test]
fn original_victim_graph_selection_retains_the_exact_fork_and_join() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_active_resident_exact_add_original_victim_chain(target);
        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.virtual_registers.len(), 10);
        assert_eq!(staged.selected().receipt().instruction_count(), 13);
        assert_eq!(function.blocks[1].instructions.len(), 8);
        assert_eq!(
            function.blocks[1].instructions[5]
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            vec![
                VirtualRegisterId(3),
                VirtualRegisterId(1),
                VirtualRegisterId(6),
            ]
        );
        assert_eq!(
            function.blocks[1].instructions[6]
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            vec![
                VirtualRegisterId(5),
                VirtualRegisterId(6),
                VirtualRegisterId(7),
            ]
        );

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[6].operands[0].virtual_register =
            VirtualRegisterId(1);
        assert_eq!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 8,
            })
        );
    }
}
