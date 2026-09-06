//! Public-validator coverage for the pressure-bearing legalized bridge chain.

use crate::tests::*;

#[test]
fn bridge_chain_legalization_is_independently_validated_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_active_resident_exact_add_bridge_chain_artifact();
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
            panic!("bridge source must use the ordinary ordered sequence")
        };
        assert_eq!(sequence.steps.len(), 7);
        let legalized_operations::LegalizedIntegerStep::ExactBinary(bridge) = &sequence.steps[5]
        else {
            panic!("the bridge is the sixth source definition")
        };
        let legalized_operations::LegalizedIntegerStep::ExactBinary(result) = &sequence.steps[6]
        else {
            panic!("the result follows the bridge")
        };
        assert_eq!(result.right, bridge.source_value);
        assert_ne!(bridge.operation, result.operation);

        let mut corrupted = legalized.plan().clone();
        let LegalizedLeafValue::ExactIntegerSequence(sequence) =
            &mut corrupted.functions[0].conditional_mut().when_true.value
        else {
            unreachable!()
        };
        sequence.steps.swap(4, 5);
        assert!(
            validate_legalized_operations(
                target.target_operations(),
                target.optimized().plan(),
                target.optimized().unit(),
                corrupted,
            )
            .is_err()
        );

        // Equivalent arithmetic does not authorize a second representation of
        // the same catalog role: its false arm remains the canonical immediate.
        let mut corrupted = legalized.plan().clone();
        let leaf = &mut corrupted.functions[0].conditional_mut().when_false;
        let LegalizedLeafValue::Immediate {
            value,
            constant_operation,
            definition_site,
            constant_fuel,
        } = &leaf.value
        else {
            unreachable!()
        };
        leaf.value = LegalizedLeafValue::ExactIntegerSequence(
            legalized_operations::LegalizedExactIntegerSequence {
                steps: vec![legalized_operations::LegalizedIntegerStep::Immediate(
                    legalized_operations::LegalizedImmediate {
                        source_value: leaf.source_value,
                        value: *value,
                        constant_operation: *constant_operation,
                        definition_site: *definition_site,
                        fuel: constant_fuel.clone(),
                    },
                )],
            },
        );
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
fn bridge_chain_selection_is_independently_replayed_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let staged = staged_active_resident_exact_add_bridge_chain(target);
        let function = &staged.selected().plan().functions[0];
        assert_eq!(function.virtual_registers.len(), 9);
        assert_eq!(staged.selected().receipt().instruction_count(), 12);
        assert_eq!(function.blocks[1].instructions.len(), 7);
        assert_eq!(
            function.blocks[1].instructions[5]
                .operands
                .iter()
                .map(|operand| operand.virtual_register)
                .collect::<Vec<_>>(),
            vec![
                VirtualRegisterId(3),
                VirtualRegisterId(5),
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
                VirtualRegisterId(1),
                VirtualRegisterId(6),
                VirtualRegisterId(7),
            ]
        );

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].blocks[1].instructions[5].operands[0].virtual_register =
            VirtualRegisterId(1);
        assert_eq!(
            validate_raw_selection(&staged, corrupted),
            Err(SelectedInstructionError::InstructionProjectionMismatch {
                function: 0,
                instruction: 7,
            })
        );

        let mut corrupted = staged.selected().plan().clone();
        corrupted.functions[0].virtual_registers[6].origin =
            corrupted.functions[0].virtual_registers[5].origin;
        assert_eq!(
            validate_raw_selection(&staged, corrupted),
            Err(
                SelectedInstructionError::VirtualRegisterProjectionMismatch {
                    function: 0,
                    register: 6,
                }
            )
        );
    }
}
