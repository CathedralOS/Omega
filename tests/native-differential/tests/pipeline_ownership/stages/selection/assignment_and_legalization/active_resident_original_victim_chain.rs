//! Public custody for the exact guarded-original prerequisite graph.

use crate::tests::*;

#[test]
fn original_victim_graph_legalization_is_distinct_and_independently_replayed() {
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
            LegalizationRecipe::ReturnU64ActiveResidentExactAddOriginalVictimChainConditionalV1,
        );
        let LegalizedLeafValue::ActiveResidentExactAddOriginalVictimChain(chain) =
            &function.when_true.value
        else {
            panic!("exact graph retains its distinct legalized carrier")
        };
        assert_ne!(chain.middle.source_value, chain.bridge.source_value);
        assert_ne!(chain.join.source_value, chain.result.source_value);

        let mut corrupted = legalized.plan().clone();
        let LegalizedLeafValue::ActiveResidentExactAddOriginalVictimChain(chain) =
            &mut corrupted.functions[0].conditional_mut().when_true.value
        else {
            unreachable!()
        };
        std::mem::swap(&mut chain.bridge.operation, &mut chain.join.operation);
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
