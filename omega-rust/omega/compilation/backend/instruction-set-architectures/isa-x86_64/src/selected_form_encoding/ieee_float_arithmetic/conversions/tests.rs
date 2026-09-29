use super::super::{decode_instruction, encode_instruction};
use super::*;
use selected_instructions::{
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation as Operation, IeeeFloatFormat,
    IeeeFloatToIntegerDomain as Domain, MachineAlternativeFamily, MachineAlternativeKey,
    MachineEncodedControlEffect, MachineEncodedTrapBehavior,
};

fn decoded(bytes: &[u8]) -> Vec<(usize, Instruction, usize)> {
    let mut instructions = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let start = bytes.len() - rest.len();
        let (instruction, tail) = decode_instruction(rest).expect("independent instruction decode");
        instructions.push((start, instruction, rest.len() - tail.len()));
        rest = tail;
    }
    instructions
}

#[test]
fn conversion_control_opcodes_have_architectural_bytes_and_independent_decode() {
    for (instruction, expected) in [
        (Instruction::Test64 { general: 0 }, vec![0x48, 0x85, 0xc0]),
        (Instruction::Test64 { general: 9 }, vec![0x4d, 0x85, 0xc9]),
        (
            Instruction::Or {
                destination: 4,
                source: 5,
            },
            vec![0x66, 0x0f, 0xeb, 0xe5],
        ),
        (
            Instruction::ConditionalJump {
                condition: NOT_ZERO,
                displacement: 2,
            },
            vec![0x0f, 0x85, 2, 0, 0, 0],
        ),
        (
            Instruction::ConditionalJump {
                condition: NOT_SIGNED,
                displacement: 17,
            },
            vec![0x0f, 0x89, 17, 0, 0, 0],
        ),
        (
            Instruction::Jump { displacement: 10 },
            vec![0xe9, 10, 0, 0, 0],
        ),
        (Instruction::Crash, vec![0x0f, 0x0b]),
    ] {
        let mut bytes = Vec::new();
        encode_instruction(&mut bytes, instruction);
        assert_eq!(bytes, expected);
        assert_eq!(decode_instruction(&expected), Some((instruction, &[][..])));
    }
}

#[test]
fn unsigned64_from_integer_uses_one_format_rounding_and_sticky_half() {
    for double in [false, true] {
        let program = from_unsigned64(double, 9, 9);
        assert!(program.contains(&Instruction::ShiftRight {
            quad: true,
            arithmetic: false,
            xmm: LEFT_SCRATCH,
            count: 1,
        }));
        assert!(program.contains(&Instruction::ShiftLeft {
            quad: true,
            xmm: RIGHT_SCRATCH,
            count: 63,
        }));
        assert!(program.contains(&Instruction::ShiftRight {
            quad: true,
            arithmetic: false,
            xmm: RIGHT_SCRATCH,
            count: 63,
        }));
        assert!(program.contains(&Instruction::Or {
            destination: LEFT_SCRATCH,
            source: RIGHT_SCRATCH,
        }));
        assert_eq!(program.iter().filter(|instruction| matches!(instruction,
            Instruction::IntegerToFloat { double: format, wide: true, .. } if *format == double
        )).count(), 2);
        assert!(program.contains(&Instruction::Scalar {
            double,
            opcode: 0x58,
            destination: LEFT_SCRATCH,
            source: LEFT_SCRATCH,
        }));
        assert!(
            !program
                .iter()
                .any(|instruction| matches!(instruction, Instruction::Scalar { opcode: 0x5a, .. })),
            "u64 to binary32 must not round through binary64"
        );
    }
    // Numerical check of the sticky-half identity at ties and adjacent values;
    // this is not a claim of executing the encoded instructions on this host.
    for value in [
        1u64 << 63,
        (1u64 << 63) + 1,
        (1u64 << 63) + 1023,
        (1u64 << 63) + 1024,
        (1u64 << 63) + 1025,
        (1u64 << 63) + (1u64 << 39) - 1,
        (1u64 << 63) + (1u64 << 39),
        (1u64 << 63) + (1u64 << 39) + 1,
        u64::MAX,
    ] {
        let half = (value >> 1) | (value & 1);
        assert_eq!(
            ((half as i64 as f32) * 2.0).to_bits(),
            (value as f32).to_bits()
        );
        assert_eq!(
            ((half as i64 as f64) * 2.0).to_bits(),
            (value as f64).to_bits()
        );
    }
}

#[test]
fn trapping_guards_replay_exact_toward_zero_intervals() {
    use IeeeFloatIntegerCarrier::*;
    for target in [I8, I16, I32, I64, U8, U16, U32, U64] {
        for double in [false, true] {
            let mut bytes = Vec::new();
            for instruction in trapping(double, target, 9, 9) {
                encode_instruction(&mut bytes, instruction);
            }
            let instructions = decoded(&bytes)
                .into_iter()
                .map(|(_, instruction, _)| instruction)
                .collect::<Vec<_>>();
            let guards = instructions
                .windows(7)
                .filter_map(|window| match window {
                    [
                        Instruction::MoveImmediate64 { general: 9, value },
                        Instruction::Transfer {
                            to_float: true,
                            wide: true,
                            xmm: RIGHT_SCRATCH,
                            general: 9,
                        },
                        Instruction::CompareScalar {
                            double: true,
                            predicate,
                            destination: RIGHT_SCRATCH,
                            source: LEFT_SCRATCH,
                        },
                        Instruction::Transfer {
                            to_float: false,
                            wide: true,
                            xmm: RIGHT_SCRATCH,
                            general: 9,
                        },
                        Instruction::Test64 { general: 9 },
                        Instruction::ConditionalJump {
                            condition,
                            displacement: 2,
                        },
                        Instruction::Crash,
                    ] => Some((f64::from_bits(*value), *predicate, *condition)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(guards.len(), 2);
            assert_eq!(guards[0].2, NOT_ZERO);
            assert_eq!((guards[1].1, guards[1].2), (LESS_EQUAL, ZERO));
            let minimum = if target.is_signed() {
                -(1i128 << (target.bits() - 1))
            } else {
                0
            };
            let maximum = (1i128 << (target.bits() - u16::from(target.is_signed()))) - 1;
            let lower = guards[0].0;
            let upper = guards[1].0;
            for operand in [
                f64::NAN,
                f64::NEG_INFINITY,
                f64::INFINITY,
                -0.0,
                0.0,
                -1.0,
                -0.75,
                1.25,
                lower - 1.0,
                lower,
                lower + 0.5,
                minimum as f64,
                f64::from_bits(lower.to_bits() + 1),
                f64::from_bits(lower.to_bits() - 1),
                f64::from_bits(upper.to_bits() - 1),
                upper,
                f64::from_bits(upper.to_bits() + 1),
            ] {
                let operand = if double {
                    operand
                } else {
                    f64::from(operand as f32)
                };
                let guards_allow = guards.iter().all(|(bound, predicate, condition)| {
                    let mask = match *predicate {
                        LESS => *bound < operand,
                        LESS_EQUAL => *bound <= operand,
                        _ => panic!("unexpected comparison"),
                    };
                    if *condition == NOT_ZERO { mask } else { !mask }
                });
                let truncated = operand.trunc() as i128;
                let expected = operand.is_finite() && truncated >= minimum && truncated <= maximum;
                assert_eq!(
                    guards_allow, expected,
                    "{target:?} double={double}, {operand:?}"
                );
            }
        }
    }
}

#[test]
fn unsigned64_saturation_replays_zero_split_and_overflow_bounds() {
    let instructions = to_unsigned64(0);
    assert!(instructions.contains(&Instruction::CompareScalar {
        double: true,
        predicate: super::super::ORDERED,
        destination: RIGHT_SCRATCH,
        source: LEFT_SCRATCH,
    }));
    assert!(
        instructions.contains(&scalar(0x5f)),
        "negative values clamp to zero"
    );
    let constants = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::MoveImmediate64 { value, .. } => Some(*value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        constants,
        [
            TWO_TO_64,
            super::super::TWO_TO_63,
            super::super::TWO_TO_63,
            u64::MAX
        ]
    );
    assert!(
        instructions.contains(&scalar(0x5c)),
        "high half subtracts 2^63 before signed truncation"
    );
    assert!(instructions.contains(&Instruction::ShiftLeft {
        quad: true,
        xmm: RIGHT_SCRATCH,
        count: 63
    }));
    assert!(instructions.contains(&Instruction::Or {
        destination: LEFT_SCRATCH,
        source: RIGHT_SCRATCH
    }));
}

#[test]
fn conversion_control_replay_binds_local_branches_aliasing_and_effects() {
    let physical =
        register_model::validate_physical_register_model(crate::x86_64_physical_register_model())
            .unwrap();
    let flags = &physical.model().view_named("rflags").unwrap().units;
    for operation in [
        Operation::FromInteger(IeeeFloatIntegerCarrier::U64),
        Operation::TruncateToInteger {
            target: IeeeFloatIntegerCarrier::U64,
            domain: Domain::Exact,
        },
        Operation::TruncateToInteger {
            target: IeeeFloatIntegerCarrier::U64,
            domain: Domain::Saturating,
        },
        Operation::TruncateToInteger {
            target: IeeeFloatIntegerCarrier::I8,
            domain: Domain::Trapping,
        },
        Operation::TruncateToInteger {
            target: IeeeFloatIntegerCarrier::U64,
            domain: Domain::Trapping,
        },
    ] {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            for register in ["rax", "r9"] {
                let form = IeeeFloatArithmeticForm { operation, format };
                let key = MachineAlternativeKey {
                    family: MachineAlternativeFamily::NearestIeeeFloatArithmetic(form),
                    variant: 0,
                };
                let view = physical.model().view_named(register).unwrap().id;
                let operands = [view, view];
                let encoded = super::super::encode(&physical, form, key, &operands).unwrap();
                let bytes = encoded.bytes();
                let instructions = decoded(bytes);
                for (position, instruction, length) in &instructions {
                    if let Instruction::ConditionalJump { displacement, .. }
                    | Instruction::Jump { displacement } = instruction
                    {
                        let destination = (*position + *length) as i32 + displacement;
                        assert!(destination >= 0 && destination as usize <= bytes.len());
                        assert!(
                            destination as usize == bytes.len()
                                || instructions
                                    .iter()
                                    .any(|(start, _, _)| *start == destination as usize)
                        );
                    }
                }
                let footprint = encoded.footprint();
                assert!(footprint.writes_rflags);
                assert!(
                    flags
                        .iter()
                        .all(|flag| footprint.encoded.implicit_unit_clobbers.contains(flag))
                );
                let traps = matches!(
                    operation,
                    Operation::TruncateToInteger {
                        domain: Domain::Trapping,
                        ..
                    }
                );
                assert_eq!(
                    footprint.encoded.trap,
                    if traps {
                        MachineEncodedTrapBehavior::TrappingIntegerV1
                    } else {
                        MachineEncodedTrapBehavior::NeverV1
                    }
                );
                assert_eq!(
                    footprint.encoded.control,
                    if traps {
                        MachineEncodedControlEffect::FallThroughOrTrapV1
                    } else {
                        MachineEncodedControlEffect::FallThroughV1
                    }
                );
                assert_eq!(
                    instructions
                        .iter()
                        .filter(|(_, instruction, _)| *instruction == Instruction::Crash)
                        .count(),
                    if traps { 2 } else { 0 }
                );
                // No relative target is accepted merely because it stays inside
                // the form: the independently decoded recipe binds its exact offset.
                for (position, instruction, _) in instructions {
                    let displacement_position = match instruction {
                        Instruction::ConditionalJump { .. } => position + 2,
                        Instruction::Jump { .. } => position + 1,
                        _ => continue,
                    };
                    let mut changed = bytes.to_vec();
                    changed[displacement_position] ^= 1;
                    assert!(
                        super::super::validate(&physical, form, key, &operands, &changed).is_err()
                    );
                }
            }
        }
    }
}
