//! Ordinary returning calls normalize their exact integer type before comparison.
use super::*;

#[test]
fn admitted_exact_casts_normalize_the_destination_integer_type() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for (sign, bits, expected) in [
            (
                IntegerSign::Unsigned,
                16,
                SelectedInstructionKind::ZeroExtendU16,
            ),
            (
                IntegerSign::Signed,
                8,
                SelectedInstructionKind::SignExtendI8,
            ),
            (
                IntegerSign::Signed,
                16,
                SelectedInstructionKind::SignExtendI16,
            ),
            (
                IntegerSign::Signed,
                32,
                SelectedInstructionKind::SignExtendI32,
            ),
        ] {
            let mut source = fixture(target, 0);
            source.blocks[0].instructions.truncate(2);
            source.provenance.operations.truncate(2);
            let row = &mut source.blocks[0].instructions[1];
            row.result.as_mut().unwrap().scalar_type =
                ScalarType::Integer(IntegerType::new(sign, bits).unwrap());
            row.kind = LegalizedScalarInstructionKind::IntegerExactCast {
                operand: ValueId::new(1).unwrap(),
                source_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                    [17; 32],
                ),
            };
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    &source,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&selected).unwrap();
            assert_eq!(selected.blocks[0].instructions[1].kind, expected);
            for replacement in [
                SelectedInstructionKind::CopyI64,
                SelectedInstructionKind::ZeroExtendU8,
                SelectedInstructionKind::ZeroExtendU32,
            ] {
                let mut changed = selected.clone();
                changed.blocks[0].instructions[1].kind = replacement;
                assert!(validate(&changed).is_err());
            }
        }
    }
}

#[test]
fn narrow_integer_call_results_preserve_comparison_types_and_reject_normalization_substitution() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            projected_structural_call: None,
            fixed_inputs: Vec::new(),
        };
        for (sign, bits, expected, wrong_sign) in [
            (
                IntegerSign::Unsigned,
                16,
                SelectedInstructionKind::ZeroExtendU16,
                SelectedInstructionKind::SignExtendI16,
            ),
            (
                IntegerSign::Signed,
                8,
                SelectedInstructionKind::SignExtendI8,
                SelectedInstructionKind::ZeroExtendU8,
            ),
            (
                IntegerSign::Signed,
                16,
                SelectedInstructionKind::SignExtendI16,
                SelectedInstructionKind::ZeroExtendU16,
            ),
            (
                IntegerSign::Signed,
                32,
                SelectedInstructionKind::SignExtendI32,
                SelectedInstructionKind::ZeroExtendU32,
            ),
        ] {
            let integer = IntegerType::new(sign, bits).unwrap();
            let shape = ValueShape::integer(bits / 8, bits / 8);
            let mut source = control::graph(
                target,
                legalized_operations::LegalizedScalarComparison::LessThan,
                false,
            );
            let entry = source
                .blocks
                .iter_mut()
                .find(|block| block.id == source.entry_block)
                .unwrap();
            for row in &mut entry.instructions {
                match &mut row.kind {
                    LegalizedScalarInstructionKind::Constant(value) => {
                        row.result.as_mut().unwrap().scalar_type = ScalarType::Integer(integer);
                        *value = if sign == IntegerSign::Signed {
                            IntegerValue::Signed(-1)
                        } else {
                            IntegerValue::Unsigned(65535)
                        };
                    }
                    LegalizedScalarInstructionKind::Call(call) => {
                        row.result.as_mut().unwrap().scalar_type = ScalarType::Integer(integer);
                        call.call_plan = evaluate_call_plan(
                            call.call_plan.policy,
                            &CallSignature {
                                parameters: vec![shape; call.arguments.len()],
                                result: Some(shape),
                            },
                        )
                        .unwrap();
                        call.result_placement = call.call_plan.result.clone();
                        for (argument, placement) in
                            call.arguments.iter_mut().zip(&call.call_plan.parameters)
                        {
                            let LegalizedScalarArgument::Scalar {
                                placement: actual, ..
                            } = argument
                            else {
                                panic!("scalar argument");
                            };
                            *actual = placement.clone();
                        }
                    }
                    LegalizedScalarInstructionKind::Compare { operand_type, .. } => {
                        *operand_type = integer
                    }
                    _ => panic!("integer comparison fixture"),
                }
            }
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
                crate::selection::validation::scalar_graph::validate(
                    0,
                    source,
                    candidate,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
            validate(&source, &selected).unwrap();
            let normalizations = selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|row| row.kind == expected)
                .count();
            assert_eq!(normalizations, 2);
            for replacement in [
                SelectedInstructionKind::CopyI64,
                wrong_sign,
                if bits == 8 {
                    SelectedInstructionKind::SignExtendI32
                } else {
                    SelectedInstructionKind::SignExtendI8
                },
            ] {
                let mut changed = selected.clone();
                changed
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.instructions)
                    .find(|row| row.kind == expected)
                    .unwrap()
                    .kind = replacement;
                assert!(validate(&source, &changed).is_err());
            }
            let mut changed = source.clone();
            let comparison = changed
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| matches!(row.kind, LegalizedScalarInstructionKind::Compare { .. }))
                .unwrap();
            let LegalizedScalarInstructionKind::Compare { operand_type, .. } = &mut comparison.kind
            else {
                panic!("comparison");
            };
            *operand_type = IntegerType::new(
                if sign == IntegerSign::Signed {
                    IntegerSign::Unsigned
                } else {
                    IntegerSign::Signed
                },
                bits,
            )
            .unwrap();
            assert!(validate(&changed, &selected).is_err());
            assert!(
                build(
                    0,
                    &changed,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints()
                )
                .is_err()
            );
        }
    }
}
