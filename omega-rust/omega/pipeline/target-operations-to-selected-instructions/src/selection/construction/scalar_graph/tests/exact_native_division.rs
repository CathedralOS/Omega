//! Exact source obligations survive selection of the concrete divide operation.
use super::{
    IntegerSign, IntegerType, LegalizedScalarInstructionKind, ScalarType, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, ValueId, build, fixture_with_integer,
};
use legalized_operations::LegalizedExactIntegerOperator;
use optimization_core::AcceptedObligationFactIdentity;
use semantic_vocabulary::{ObligationId, OperationId};

#[test]
fn exact_native_division_preserves_policy_proof_and_register_custody() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [8, 16, 32, 64] {
                for operator in [
                    LegalizedExactIntegerOperator::Divide,
                    LegalizedExactIntegerOperator::Remainder,
                ] {
                    for shared_operand in [false, true] {
                        let integer = IntegerType::new(sign, bits).unwrap();
                        let mut source = fixture_with_integer(target, 0, integer);
                        source.blocks[0].instructions.truncate(3);
                        source.provenance.operations.truncate(3);
                        let obligation = ObligationId::new(1).unwrap();
                        let accepted_fact = AcceptedObligationFactIdentity::from_bytes([41; 32]);
                        source.blocks[0].instructions[2].kind =
                            LegalizedScalarInstructionKind::ExactBinary {
                                operator,
                                left: ValueId::new(1).unwrap(),
                                right: ValueId::new(if shared_operand { 1 } else { 2 }).unwrap(),
                                obligation,
                                accepted_fact,
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
                        let instruction_position = selected.blocks[0]
                            .instructions
                            .iter()
                            .position(|instruction| {
                                instruction.provenance.operations == [OperationId::new(3).unwrap()]
                            })
                            .unwrap();
                        let unsigned = sign == IntegerSign::Unsigned && bits == 64;
                        let expected = match (unsigned, operator) {
                            (false, LegalizedExactIntegerOperator::Divide) => {
                                SelectedInstructionKind::ExactDivideI64 {
                                    obligation,
                                    accepted_fact,
                                }
                            }
                            (false, _) => SelectedInstructionKind::ExactRemainderI64 {
                                obligation,
                                accepted_fact,
                            },
                            (true, LegalizedExactIntegerOperator::Divide) => {
                                SelectedInstructionKind::ExactDivideU64 {
                                    obligation,
                                    accepted_fact,
                                }
                            }
                            (true, _) => SelectedInstructionKind::ExactRemainderU64 {
                                obligation,
                                accepted_fact,
                            },
                        };
                        assert_eq!(
                            selected.blocks[0].instructions[instruction_position].kind,
                            expected
                        );
                        for corruption in 0..6 {
                            let mut changed = selected.clone();
                            let instruction =
                                &mut changed.blocks[0].instructions[instruction_position];
                            match corruption {
                                0 => instruction.provenance.obligations.clear(),
                                1 => instruction.provenance.operations.clear(),
                                2 => match &mut instruction.kind {
                                    SelectedInstructionKind::ExactDivideI64 {
                                        obligation, ..
                                    }
                                    | SelectedInstructionKind::ExactRemainderI64 {
                                        obligation,
                                        ..
                                    }
                                    | SelectedInstructionKind::ExactDivideU64 {
                                        obligation, ..
                                    }
                                    | SelectedInstructionKind::ExactRemainderU64 {
                                        obligation,
                                        ..
                                    } => *obligation = ObligationId::new(2).unwrap(),
                                    _ => unreachable!(),
                                },
                                3 => match &mut instruction.kind {
                                    SelectedInstructionKind::ExactDivideI64 {
                                        accepted_fact,
                                        ..
                                    }
                                    | SelectedInstructionKind::ExactRemainderI64 {
                                        accepted_fact,
                                        ..
                                    }
                                    | SelectedInstructionKind::ExactDivideU64 {
                                        accepted_fact,
                                        ..
                                    }
                                    | SelectedInstructionKind::ExactRemainderU64 {
                                        accepted_fact,
                                        ..
                                    } => {
                                        *accepted_fact =
                                            AcceptedObligationFactIdentity::from_bytes([42; 32])
                                    }
                                    _ => unreachable!(),
                                },
                                4 => {
                                    instruction.kind = SelectedInstructionKind::ExactAddI64 {
                                        obligation,
                                        accepted_fact,
                                    }
                                }
                                _ => {
                                    let output = instruction.operands[2].virtual_register;
                                    let wrong_sign = if sign == IntegerSign::Signed {
                                        IntegerSign::Unsigned
                                    } else {
                                        IntegerSign::Signed
                                    };
                                    changed.virtual_registers[output.0 as usize].scalar_type =
                                        ScalarType::Integer(
                                            IntegerType::new(wrong_sign, bits).unwrap(),
                                        );
                                }
                            }
                            assert!(
                                validate(&changed).is_err(),
                                "{target:?}/{sign:?}/{bits}/{operator:?}/{shared_operand}: corruption {corruption}"
                            );
                        }
                        if target.architecture == target::Architecture::X86_64 {
                            let mut changed = selected.clone();
                            changed.blocks[0].instructions[instruction_position]
                                .operands
                                .pop();
                            assert!(validate(&changed).is_err(), "missing x86 high-half scratch");
                            if shared_operand
                                && (!unsigned
                                    || operator == LegalizedExactIntegerOperator::Remainder)
                            {
                                let instruction =
                                    &selected.blocks[0].instructions[instruction_position];
                                assert_ne!(
                                    instruction.operands[0].virtual_register,
                                    instruction.operands[1].virtual_register
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
