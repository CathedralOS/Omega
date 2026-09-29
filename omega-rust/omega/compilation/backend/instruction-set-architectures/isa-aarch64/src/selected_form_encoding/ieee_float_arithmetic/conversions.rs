//! Trapping truncation checks the mathematical integer destination before
//! conversion. Binary32 widens exactly, so one binary64 bounds check covers
//! both formats without rounding a narrow carrier's fractional lower edge.
//! Ordered vector comparisons reject NaN and preserve NZCV; the existing
//! scratch vectors and output general register suffice, including aliasing.

use super::{
    IeeeFloatIntegerCarrier, LEFT_SCRATCH, MASK_SCRATCH, RIGHT_SCRATCH, Word, materialize,
};

/// Open binary64 bounds on every source whose truncation fits the carrier.
fn bounds(target: IeeeFloatIntegerCarrier) -> (u64, u64) {
    let magnitude_bits = u32::from(target.bits()) - u32::from(target.is_signed());
    let upper = u64::from(1023 + magnitude_bits) << 52;
    let lower = if target.is_signed() {
        // For i8/i16/i32 this is min - 1, admitting valid negative fractions.
        // For i64 min - 1 is not representable: the predecessor of min gives
        // the same predicate as >= min on binary64 (and widened binary32).
        (1_u64 << 63)
            | upper
            | if magnitude_bits <= 52 {
                1_u64 << (52 - magnitude_bits)
            } else {
                1
            }
    } else {
        (-1.0_f64).to_bits()
    };
    (lower, upper)
}

pub(super) fn trapping_truncation(
    double: bool,
    target: IeeeFloatIntegerCarrier,
    source: u8,
    result: u8,
) -> Vec<Word> {
    let mut words = vec![Word::ToFloat {
        double,
        general: source,
        vector: LEFT_SCRATCH,
    }];
    if !double {
        words.push(Word::OneSource {
            double: false,
            opcode: 0b00_0101,
            source: LEFT_SCRATCH,
            result: LEFT_SCRATCH,
        });
    }
    let (lower, upper) = bounds(target);
    for (bound, left, right) in [
        (lower, LEFT_SCRATCH, RIGHT_SCRATCH),
        (upper, RIGHT_SCRATCH, LEFT_SCRATCH),
    ] {
        words.extend(materialize(bound, result));
        words.extend([
            Word::ToFloat {
                double: true,
                general: result,
                vector: RIGHT_SCRATCH,
            },
            Word::Compare {
                equal: false,
                double: true,
                left,
                right,
                result: MASK_SCRATCH,
            },
            Word::ToGeneral {
                double: true,
                vector: MASK_SCRATCH,
                general: result,
            },
            Word::BranchNonzeroOverCrash { general: result },
            Word::Crash,
        ]);
    }
    words.push(Word::FloatToInteger {
        wide: target.bits() == 64,
        double: true,
        unsigned: !target.is_signed(),
        vector: LEFT_SCRATCH,
        general: result,
    });
    if target.is_signed() && target.bits() <= 32 {
        words.push(Word::SignExtend32 {
            source: result,
            result,
        });
    }
    words
}

#[cfg(test)]
mod tests {
    use super::super::{declaration, decode_word, encode, footprint, validate};
    use super::*;
    use selected_instructions::{
        IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatFormat,
        IeeeFloatToIntegerDomain, MachineBarrier, MachineEncodedControlEffect,
        MachineEncodedTrapBehavior, MachineSizeKnowledge, MachineTrapBehavior,
    };

    fn form(target: IeeeFloatIntegerCarrier, format: IeeeFloatFormat) -> IeeeFloatArithmeticForm {
        IeeeFloatArithmeticForm {
            operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                target,
                domain: IeeeFloatToIntegerDomain::Trapping,
            },
            format,
        }
    }

    #[test]
    fn trapping_words_pin_the_bounds_branches_and_conversion() {
        // fmov d16,x3; movz x4,#0xc1e0,lsl#48; movk x4,#0x20,lsl#16;
        // fmov d17,x4; fcmgt d18,d16,d17; fmov x4,d18; cbnz x4,.+8; brk#0;
        // movz x4,#0x41e0,lsl#48; fmov d17,x4; fcmgt d18,d17,d16;
        // fmov x4,d18; cbnz x4,.+8; brk#0; fcvtzs w4,d16; sxtw x4,w4.
        let selected = form(IeeeFloatIntegerCarrier::I32, IeeeFloatFormat::Binary64);
        let expected = [
            0x9e67_0070_u32,
            0xd2f8_3c04,
            0xf2a0_0404,
            0x9e67_0091,
            0x7ef1_e612,
            0x9e66_0244,
            0xb500_0044,
            0xd420_0000,
            0xd2e8_3c04,
            0x9e67_0091,
            0x7ef0_e632,
            0x9e66_0244,
            0xb500_0044,
            0xd420_0000,
            0x1e78_0204,
            0x9340_7c84,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        assert_eq!(encode(selected, &[3, 4]).unwrap(), expected);
        validate(selected, &[3, 4], &expected).unwrap();
        let mut changed = expected.clone();
        changed[6 * 4..7 * 4].copy_from_slice(&0xb500_0084_u32.to_le_bytes());
        assert!(validate(selected, &[3, 4], &changed).is_err());
        changed = expected;
        changed[7 * 4..8 * 4].copy_from_slice(&0xd503_201f_u32.to_le_bytes());
        assert!(validate(selected, &[3, 4], &changed).is_err());
    }

    #[test]
    fn unsigned_64_conversion_words_select_the_unsigned_wide_carrier() {
        for (format, to_float, from_float, move_in, move_out) in [
            (
                IeeeFloatFormat::Binary32,
                0x9e23_0070_u32,
                0x9e39_0204_u32,
                0x1e27_0070_u32,
                0x1e26_0204_u32,
            ),
            (
                IeeeFloatFormat::Binary64,
                0x9e63_0070,
                0x9e79_0204,
                0x9e67_0070,
                0x9e66_0204,
            ),
        ] {
            let selected = IeeeFloatArithmeticForm {
                operation: IeeeFloatArithmeticOperation::FromInteger(IeeeFloatIntegerCarrier::U64),
                format,
            };
            assert_eq!(
                encode(selected, &[3, 4]).unwrap(),
                [to_float, move_out]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            );
            for domain in [
                IeeeFloatToIntegerDomain::Exact,
                IeeeFloatToIntegerDomain::Saturating,
            ] {
                let selected = IeeeFloatArithmeticForm {
                    operation: IeeeFloatArithmeticOperation::TruncateToInteger {
                        target: IeeeFloatIntegerCarrier::U64,
                        domain,
                    },
                    format,
                };
                assert_eq!(
                    encode(selected, &[3, 4]).unwrap(),
                    [move_in, from_float]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn trapping_footprint_declares_crash_without_flags_or_extra_scratch() {
        let model = crate::aarch64_physical_register_model();
        let operands = [
            model.view_named("x3").unwrap().id,
            model.view_named("x4").unwrap().id,
        ];
        for target in IeeeFloatIntegerCarrier::ALL {
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                let selected = form(target, format);
                let declaration = declaration(selected, crate::AARCH64_IEEE_FLOAT_UNARY);
                let footprint = footprint(selected, &operands);
                assert_eq!(declaration.trap, MachineTrapBehavior::TrappingIntegerV1);
                assert_eq!(declaration.barrier, MachineBarrier::ExternalEffect);
                assert_eq!(
                    footprint.encoded.trap,
                    MachineEncodedTrapBehavior::TrappingIntegerV1
                );
                assert_eq!(
                    footprint.encoded.control,
                    MachineEncodedControlEffect::FallThroughOrTrapV1
                );
                assert_eq!(footprint.register_reads, operands[..1]);
                assert_eq!(footprint.register_writes, operands[1..]);
                assert!(!footprint.writes_nzcv);
                assert_eq!(
                    footprint.encoded.implicit_unit_clobbers,
                    super::super::clobbers()
                );
                assert_eq!(declaration.alternatives[0].encoded, footprint.encoded);
                assert_eq!(
                    declaration.alternatives[0].size,
                    MachineSizeKnowledge::ExactBytes(
                        encode(selected, &[3, 4]).unwrap().len() as u16
                    )
                );
            }
        }
    }

    /// Replay independently decoded instructions, including the actual branch
    /// and Crash. The reference below instead truncates into i128 and checks
    /// integer bounds; it does not use the encoder's floating thresholds.
    fn replay(bytes: &[u8], source: u64, input: usize, output: usize) -> Option<u64> {
        let mut general = [0_u64; 31];
        let mut vector = [0_u64; 32];
        general[input] = source;
        let words = bytes.as_chunks::<4>().0;
        let mut position = 0;
        while position < words.len() {
            match decode_word(u32::from_le_bytes(words[position])).unwrap() {
                Word::ToFloat {
                    double,
                    general: source,
                    vector: destination,
                } => {
                    vector[usize::from(destination)] = if double {
                        general[usize::from(source)]
                    } else {
                        u64::from(general[usize::from(source)] as u32)
                    };
                }
                Word::ToGeneral {
                    double: true,
                    vector: source,
                    general: destination,
                } => general[usize::from(destination)] = vector[usize::from(source)],
                Word::OneSource {
                    double: false,
                    opcode: 0b00_0101,
                    source,
                    result,
                } => {
                    vector[usize::from(result)] =
                        f64::from(f32::from_bits(vector[usize::from(source)] as u32)).to_bits()
                }
                Word::MoveWide {
                    keep,
                    shift,
                    immediate,
                    general: destination,
                } => {
                    let shift = u32::from(shift) * 16;
                    let preserved = if keep {
                        general[usize::from(destination)] & !(0xffff_u64 << shift)
                    } else {
                        0
                    };
                    general[usize::from(destination)] = preserved | (u64::from(immediate) << shift);
                }
                Word::Compare {
                    equal: false,
                    double: true,
                    left,
                    right,
                    result,
                } => {
                    vector[usize::from(result)] = if f64::from_bits(vector[usize::from(left)])
                        > f64::from_bits(vector[usize::from(right)])
                    {
                        u64::MAX
                    } else {
                        0
                    }
                }
                Word::BranchNonzeroOverCrash { general: source } => {
                    if general[usize::from(source)] != 0 {
                        position += 1;
                    }
                }
                Word::Crash => return None,
                Word::FloatToInteger {
                    wide,
                    double: true,
                    unsigned,
                    vector: source,
                    general: destination,
                } => {
                    let source = f64::from_bits(vector[usize::from(source)]);
                    general[usize::from(destination)] = match (wide, unsigned) {
                        (true, true) => source as u64,
                        (true, false) => source as i64 as u64,
                        (false, true) => u64::from(source as u32),
                        (false, false) => u64::from(source as i32 as u32),
                    };
                }
                Word::SignExtend32 { source, result } => {
                    general[usize::from(result)] = general[usize::from(source)] as i32 as i64 as u64
                }
                other => panic!("unexpected trapping conversion instruction {other:?}"),
            }
            position += 1;
        }
        Some(general[output])
    }

    #[test]
    fn trapping_replay_matches_integer_bounds_at_fractional_edges_and_nonfinite_values() {
        for target in IeeeFloatIntegerCarrier::ALL {
            let magnitude_bits = target.bits() - u16::from(target.is_signed());
            let maximum = (1_i128 << magnitude_bits) - 1;
            let minimum = if target.is_signed() {
                -(1_i128 << magnitude_bits)
            } else {
                0
            };
            let mut sources = vec![
                f64::NAN,
                f64::NEG_INFINITY,
                f64::INFINITY,
                -0.0,
                0.0,
                -0.75,
                0.75,
                -1.0,
                -1.5,
                1.5,
            ];
            for edge in [
                minimum as f64,
                maximum as f64,
                (minimum - 1) as f64,
                (maximum + 1) as f64,
            ] {
                sources.extend([
                    edge,
                    edge.next_down(),
                    edge.next_up(),
                    f64::from((edge as f32).next_down()),
                    f64::from((edge as f32).next_up()),
                    edge - 0.5,
                    edge + 0.5,
                ]);
            }
            for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
                for &source in &sources {
                    let (source, payload) = match format {
                        IeeeFloatFormat::Binary32 => (
                            f64::from(source as f32),
                            u64::from((source as f32).to_bits()),
                        ),
                        IeeeFloatFormat::Binary64 => (source, source.to_bits()),
                    };
                    let integer = source.trunc() as i128;
                    let expected = (source.is_finite() && (minimum..=maximum).contains(&integer))
                        .then_some(integer as u64);
                    for registers in [[3, 4], [3, 3]] {
                        let bytes = encode(form(target, format), &registers).unwrap();
                        assert_eq!(
                            replay(
                                &bytes,
                                payload,
                                usize::from(registers[0]),
                                usize::from(registers[1])
                            ),
                            expected,
                            "{target:?} {format:?} {source} {registers:?}"
                        );
                    }
                }
            }
        }
    }
}
