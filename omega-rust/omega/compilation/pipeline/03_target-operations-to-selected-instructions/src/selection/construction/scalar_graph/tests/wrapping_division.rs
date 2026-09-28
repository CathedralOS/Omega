//! Wrapping divide and remainder select at every fixed native width: u64
//! takes the unsigned rows, every other carrier the signed i64 rows, and a
//! narrow signed quotient re-normalizes so its widened MIN / -1 wraps back
//! to the carrier MIN.
use super::{
    IntegerSign, IntegerType, LegalizedScalarInstructionKind, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, ValueId, build, fixture_with_integer,
};
use optimization_core::AcceptedObligationFactIdentity;
use semantic_vocabulary::{ObligationId, OperationId, ScalarType};

#[test]
fn wrapping_division_selects_every_width_and_wraps_narrow_signed_quotients() {
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
        let x86 = target.architecture == target::Architecture::X86_64;
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [8, 16, 32, 64] {
                for divides in [true, false] {
                    for shared_operand in [false, true] {
                        let integer = IntegerType::new(sign, bits).unwrap();
                        let obligation = ObligationId::new(1).unwrap();
                        let accepted_fact = AcceptedObligationFactIdentity::from_bytes([41; 32]);
                        let (left, right) = (
                            ValueId::new(1).unwrap(),
                            ValueId::new(if shared_operand { 1 } else { 2 }).unwrap(),
                        );
                        let mut source = fixture_with_integer(target, 0, integer);
                        source.blocks[0].instructions.truncate(3);
                        source.provenance.operations.truncate(3);
                        source.blocks[0].instructions[2].kind = if divides {
                            LegalizedScalarInstructionKind::WrappingDivide {
                                left,
                                right,
                                obligation,
                                accepted_fact,
                            }
                        } else {
                            LegalizedScalarInstructionKind::WrappingRemainder {
                                left,
                                right,
                                obligation,
                                accepted_fact,
                            }
                        };
                        let selected = build(
                            0,
                            &source,
                            target,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{target:?} {sign:?} {bits} divides={divides}: {error:?}")
                        });
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
                        let position = selected.blocks[0]
                            .instructions
                            .iter()
                            .position(|instruction| {
                                instruction.provenance.operations == [OperationId::new(3).unwrap()]
                            })
                            .unwrap();
                        let u64_carrier = sign == IntegerSign::Unsigned && bits == 64;
                        let (expected, key) = match (divides, u64_carrier) {
                            (true, true) => (
                                SelectedInstructionKind::ExactDivideU64 {
                                    obligation,
                                    accepted_fact,
                                },
                                constraints.keys.divide_u64,
                            ),
                            (true, false) => (
                                SelectedInstructionKind::WrappingDivideI64 {
                                    obligation,
                                    accepted_fact,
                                },
                                constraints.keys.divide_i64,
                            ),
                            (false, true) => (
                                SelectedInstructionKind::ExactRemainderU64 {
                                    obligation,
                                    accepted_fact,
                                },
                                constraints.keys.remainder_u64,
                            ),
                            (false, false) => (
                                SelectedInstructionKind::WrappingRemainderI64 {
                                    obligation,
                                    accepted_fact,
                                },
                                constraints.keys.remainder_i64,
                            ),
                        };
                        let instruction = &selected.blocks[0].instructions[position];
                        assert_eq!(instruction.kind, expected);
                        assert_eq!(instruction.constraint, key);
                        assert_eq!(instruction.provenance.obligations, [obligation]);
                        // Only a narrow signed quotient can leave its carrier
                        // (MIN / -1 widens to -MIN); its extension truncates
                        // the raw quotient back to the wrapped MIN.
                        let normalizes = divides && sign == IntegerSign::Signed && bits < 64;
                        let next = selected.blocks[0].instructions.get(position + 1);
                        if normalizes {
                            let next = next.unwrap();
                            assert_eq!(
                                next.kind,
                                match bits {
                                    8 => SelectedInstructionKind::SignExtendI8,
                                    16 => SelectedInstructionKind::SignExtendI16,
                                    _ => SelectedInstructionKind::SignExtendI32,
                                }
                            );
                            assert_eq!(
                                next.operands[0].virtual_register,
                                instruction.operands[2].virtual_register
                            );
                            assert_eq!(next.provenance.values, [ValueId::new(3).unwrap()]);
                            for wrong in [
                                SelectedInstructionKind::CopyI64,
                                SelectedInstructionKind::ZeroExtendU32,
                                if bits == 32 {
                                    SelectedInstructionKind::SignExtendI16
                                } else {
                                    SelectedInstructionKind::SignExtendI32
                                },
                            ] {
                                let mut changed = selected.clone();
                                changed.blocks[0].instructions[position + 1].kind = wrong;
                                assert!(
                                    validate(&changed).is_err(),
                                    "{target:?} {bits}: {wrong:?} accepted as the carrier wrap"
                                );
                            }
                        } else {
                            assert!(next.is_none_or(
                                |next| next.provenance.values != [ValueId::new(3).unwrap()]
                            ));
                        }
                        for corruption in 0..4 {
                            let mut changed = selected.clone();
                            let instruction = &mut changed.blocks[0].instructions[position];
                            match corruption {
                                0 => instruction.provenance.obligations.clear(),
                                1 => {
                                    instruction.kind = match (divides, u64_carrier) {
                                        (true, true) => {
                                            SelectedInstructionKind::ExactRemainderU64 {
                                                obligation,
                                                accepted_fact,
                                            }
                                        }
                                        (true, false) => SelectedInstructionKind::ExactDivideI64 {
                                            obligation,
                                            accepted_fact,
                                        },
                                        (false, true) => SelectedInstructionKind::ExactDivideU64 {
                                            obligation,
                                            accepted_fact,
                                        },
                                        (false, false) => {
                                            SelectedInstructionKind::ExactRemainderI64 {
                                                obligation,
                                                accepted_fact,
                                            }
                                        }
                                    }
                                }
                                2 => {
                                    instruction.kind = match instruction.kind {
                                        SelectedInstructionKind::ExactDivideU64 { .. } => {
                                            SelectedInstructionKind::ExactDivideU64 {
                                                obligation: ObligationId::new(2).unwrap(),
                                                accepted_fact,
                                            }
                                        }
                                        SelectedInstructionKind::WrappingDivideI64 { .. } => {
                                            SelectedInstructionKind::WrappingDivideI64 {
                                                obligation,
                                                accepted_fact:
                                                    AcceptedObligationFactIdentity::from_bytes(
                                                        [42; 32],
                                                    ),
                                            }
                                        }
                                        SelectedInstructionKind::ExactRemainderU64 { .. } => {
                                            SelectedInstructionKind::ExactRemainderU64 {
                                                obligation: ObligationId::new(2).unwrap(),
                                                accepted_fact,
                                            }
                                        }
                                        _ => SelectedInstructionKind::WrappingRemainderI64 {
                                            obligation,
                                            accepted_fact:
                                                AcceptedObligationFactIdentity::from_bytes([42; 32]),
                                        },
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
                                "{target:?} {sign:?} {bits} divides={divides} shared={shared_operand}: corruption {corruption}"
                            );
                        }
                        if x86 {
                            let mut changed = selected.clone();
                            changed.blocks[0].instructions[position].operands.pop();
                            assert!(validate(&changed).is_err(), "missing x86 high-half scratch");
                        }
                    }
                }
            }
        }
    }
}
