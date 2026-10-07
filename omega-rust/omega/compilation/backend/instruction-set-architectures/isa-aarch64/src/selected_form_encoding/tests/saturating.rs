//! Saturating-form tests: register/condition/flag bindings, the
//! clang-assembled carrier roster, realization sharing, aliased-output
//! rejection, and decoded-word edge clamps against the executed
//! reference.

use super::*;

#[test]
fn saturating_subtract_binds_registers_condition_and_flag_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingSubtract {
        carrier: SaturatingCarrier::U64,
    };
    let key = alternative(MachineAlternativeFamily::SaturatingSubtract(
        SaturatingCarrier::U64,
    ));
    for left in ["x0", "x1", "x9", "x29"] {
        for right in ["x0", "x1", "x9", "x29"] {
            for destination in ["x0", "x1", "x9", "x29"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands);

                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 8);
                let decoded = validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(!decoded.footprint().encoded.implicit_unit_defs.is_empty());
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "changed byte {byte}"
                    );
                }
            }
        }
    }
}

#[test]
fn saturating_add_binds_registers_condition_and_flag_effects() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let kind = SelectedInstructionKind::SaturatingAdd {
        carrier: SaturatingCarrier::U64,
    };
    let key = alternative(MachineAlternativeFamily::SaturatingAdd(
        SaturatingCarrier::U64,
    ));
    for left in ["x0", "x1", "x9", "x29"] {
        for right in ["x0", "x1", "x9", "x29"] {
            for destination in ["x0", "x1", "x9", "x29"] {
                let operands = [left, right, destination]
                    .map(|name| physical.model().view_named(name).unwrap().id);
                let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands);

                let encoded = encoded.unwrap();
                assert_eq!(encoded.bytes().len(), 8);
                let decoded = validate_aarch64_selected_form_encoding(
                    &physical,
                    kind,
                    key,
                    &operands,
                    encoded.bytes(),
                )
                .unwrap();
                assert_eq!(decoded.footprint().encoded.external_operand_reads, [0, 1]);
                assert_eq!(decoded.footprint().encoded.external_operand_writes, [2]);
                assert!(!decoded.footprint().encoded.implicit_unit_defs.is_empty());
                for byte in 0..encoded.bytes().len() {
                    let mut changed = encoded.bytes().to_vec();
                    changed[byte] ^= 1;
                    assert!(
                        validate_aarch64_selected_form_encoding(
                            &physical, kind, key, &operands, &changed
                        )
                        .is_err(),
                        "changed byte {byte}"
                    );
                }
            }
        }
    }
}

#[test]
fn saturation_high_register_bytes_match_independent_assembler() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let operands = ["x9", "x10", "x11"].map(|name| physical.model().view_named(name).unwrap().id);
    // Independently assembled with Apple clang; not derived from this encoder.
    for (kind, family, bytes) in [
        (
            SelectedInstructionKind::SaturatingSubtract {
                carrier: SaturatingCarrier::U64,
            },
            MachineAlternativeFamily::SaturatingSubtract(SaturatingCarrier::U64),
            [0xeb0a012bu32, 0x9a9f216b]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
        (
            SelectedInstructionKind::SaturatingAdd {
                carrier: SaturatingCarrier::U64,
            },
            MachineAlternativeFamily::SaturatingAdd(SaturatingCarrier::U64),
            [0xab0a012bu32, 0xda9f316b]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ),
    ] {
        let key = alternative(family);
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands).unwrap();
        assert_eq!(encoded.bytes(), bytes);
        validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &bytes).unwrap();
    }
}

/// Every saturating kind with its family, over all realized carriers.
fn saturating_kinds() -> Vec<(
    SaturatingOperation,
    SaturatingCarrier,
    SelectedInstructionKind,
    MachineAlternativeFamily,
)> {
    let mut kinds = Vec::new();
    for carrier in SaturatingCarrier::ALL {
        kinds.push((
            SaturatingOperation::Add,
            carrier,
            SelectedInstructionKind::SaturatingAdd { carrier },
            MachineAlternativeFamily::SaturatingAdd(carrier),
        ));
        kinds.push((
            SaturatingOperation::Subtract,
            carrier,
            SelectedInstructionKind::SaturatingSubtract { carrier },
            MachineAlternativeFamily::SaturatingSubtract(carrier),
        ));
        kinds.push((
            SaturatingOperation::Divide,
            carrier,
            SelectedInstructionKind::SaturatingDivide {
                carrier,
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            MachineAlternativeFamily::SaturatingDivide(carrier),
        ));
        kinds.push((
            SaturatingOperation::Remainder,
            carrier,
            SelectedInstructionKind::SaturatingRemainder {
                carrier,
                obligation: ObligationId::new(1).unwrap(),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            },
            MachineAlternativeFamily::SaturatingRemainder(carrier),
        ));
        kinds.push((
            SaturatingOperation::Multiply,
            carrier,
            SelectedInstructionKind::SaturatingMultiply { carrier },
            MachineAlternativeFamily::SaturatingMultiply(carrier),
        ));
    }
    kinds
}

/// Every saturating form over `x1, x2 -> x3` with bound scratch `x4`,
/// independently assembled with Apple clang 17; not derived from this encoder.
const CLANG_SATURATING_WORDS: [(SaturatingOperation, SaturatingCarrier, &[u32]); 40] = [
    (
        SaturatingOperation::Add,
        SaturatingCarrier::I8,
        &[
            0x8b02_0023,
            0xb240_1be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb279_e3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::I16,
        &[
            0x8b02_0023,
            0xb240_3be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb271_c3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::I32,
        &[
            0x8b02_0023,
            0xb240_7be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb261_83e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::I64,
        &[0x937f_fc24, 0xd240_f884, 0xab02_0023, 0x9a83_6083],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::U8,
        &[0x8b02_0023, 0xb240_1fe4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::U16,
        &[0x8b02_0023, 0xb240_3fe4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::U32,
        &[0x8b02_0023, 0xb240_7fe4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Add,
        SaturatingCarrier::U64,
        &[0xab02_0023, 0xda9f_3063],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::I8,
        &[
            0xcb02_0023,
            0xb240_1be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb279_e3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::I16,
        &[
            0xcb02_0023,
            0xb240_3be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb271_c3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::I32,
        &[
            0xcb02_0023,
            0xb240_7be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb261_83e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::I64,
        &[0x937f_fc24, 0xd240_f884, 0xeb02_0023, 0x9a83_6083],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::U8,
        &[0xeb02_0023, 0x9a9f_2063],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::U16,
        &[0xeb02_0023, 0x9a9f_2063],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::U32,
        &[0xeb02_0023, 0x9a9f_2063],
    ),
    (
        SaturatingOperation::Subtract,
        SaturatingCarrier::U64,
        &[0xeb02_0023, 0x9a9f_2063],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::I8,
        &[0x9ac2_0c23, 0xb240_1be4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::I16,
        &[0x9ac2_0c23, 0xb240_3be4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::I32,
        &[0x9ac2_0c23, 0xb240_7be4, 0xeb04_007f, 0x9a83_c083],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::I64,
        &[
            0x9ac2_0c23,
            0xb241_03e4,
            0xeb04_007f,
            0xba41_0840,
            0xb240_fbe4,
            0x9a83_0083,
        ],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::U8,
        &[0x9ac2_0823],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::U16,
        &[0x9ac2_0823],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::U32,
        &[0x9ac2_0823],
    ),
    (
        SaturatingOperation::Divide,
        SaturatingCarrier::U64,
        &[0x9ac2_0823],
    ),
    // sdiv/udiv x3, x1, x2 then msub x3, x3, x2, x1; the remainder needs no
    // clamp and the wrapped MIN / -1 quotient still recovers zero.
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::I8,
        &[0x9ac2_0c23, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::I16,
        &[0x9ac2_0c23, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::I32,
        &[0x9ac2_0c23, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::I64,
        &[0x9ac2_0c23, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::U8,
        &[0x9ac2_0823, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::U16,
        &[0x9ac2_0823, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::U32,
        &[0x9ac2_0823, 0x9b02_8463],
    ),
    (
        SaturatingOperation::Remainder,
        SaturatingCarrier::U64,
        &[0x9ac2_0823, 0x9b02_8463],
    ),
    // mul x3, x1, x2 then the narrow clamps; the unsigned carriers select
    // their maximum on HI because a u32 product may pass i64::MAX.
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::I8,
        &[
            0x9b02_7c23,
            0xb240_1be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb279_e3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::I16,
        &[
            0x9b02_7c23,
            0xb240_3be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb271_c3e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::I32,
        &[
            0x9b02_7c23,
            0xb240_7be4,
            0xeb04_007f,
            0x9a83_c083,
            0xb261_83e4,
            0xeb04_007f,
            0x9a83_b083,
        ],
    ),
    // smulh x4, x1, x2; mul x3, x1, x2; cmp x4, x3, asr #63; asr x4, x4,
    // #63; eor x4, x4, #0x7fffffffffffffff; csel x3, x4, x3, ne.
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::I64,
        &[
            0x9b42_7c24,
            0x9b02_7c23,
            0xeb83_fc9f,
            0x937f_fc84,
            0xd240_f884,
            0x9a83_1083,
        ],
    ),
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::U8,
        &[0x9b02_7c23, 0xb240_1fe4, 0xeb04_007f, 0x9a83_8083],
    ),
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::U16,
        &[0x9b02_7c23, 0xb240_3fe4, 0xeb04_007f, 0x9a83_8083],
    ),
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::U32,
        &[0x9b02_7c23, 0xb240_7fe4, 0xeb04_007f, 0x9a83_8083],
    ),
    // umulh x4, x1, x2; mul x3, x1, x2; cmp x4, #0; csinv x3, x3, xzr, eq.
    (
        SaturatingOperation::Multiply,
        SaturatingCarrier::U64,
        &[0x9bc2_7c24, 0x9b02_7c23, 0xf100_009f, 0xda9f_0063],
    ),
];

fn clang_words(operation: SaturatingOperation, carrier: SaturatingCarrier) -> Vec<u8> {
    CLANG_SATURATING_WORDS
        .iter()
        .find(|(row_operation, row_carrier, _)| {
            *row_operation == operation && *row_carrier == carrier
        })
        .map(|(_, _, words)| words.iter().copied().flat_map(u32::to_le_bytes).collect())
        .unwrap()
}

#[test]
fn every_saturating_carrier_matches_independent_assembler_and_rejects_mutation() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    let names = ["x1", "x2", "x3", "x4"];
    for (operation, carrier, kind, family) in saturating_kinds() {
        let key = alternative(family);
        let expected = clang_words(operation, carrier);
        let operand_count = SaturatingRealization::of(operation, carrier).operand_count();
        let operands: Vec<_> = names[..operand_count]
            .iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect();
        let encoded = encode_aarch64_selected_form(&physical, kind, key, &operands)
            .unwrap_or_else(|error| panic!("{kind:?}: {error:?}"));
        assert_eq!(encoded.bytes(), expected, "{kind:?}");
        assert_eq!(encoded.footprint().register_reads, operands[..2]);
        assert_eq!(encoded.footprint().register_writes, operands[2..]);
        assert_eq!(
            encoded.footprint().writes_nzcv,
            !(operation == SaturatingOperation::Divide && !carrier.is_signed())
                && operation != SaturatingOperation::Remainder,
            "{kind:?}"
        );
        assert_eq!(encoded.footprint().encoded.external_operand_reads, [0, 1]);
        assert_eq!(
            encoded.footprint().encoded.external_operand_writes,
            (2..operand_count as u16).collect::<Vec<_>>()
        );
        for byte_position in 0..expected.len() {
            let mut changed = expected.clone();
            changed[byte_position] ^= 1;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &operands, &changed)
                    .is_err(),
                "{kind:?} mutated byte {byte_position}"
            );
        }
        for operand_position in 0..operands.len() {
            let mut changed = operands.clone();
            changed[operand_position] = physical.model().view_named("x8").unwrap().id;
            assert!(
                validate_aarch64_selected_form_encoding(&physical, kind, key, &changed, &expected)
                    .is_err(),
                "{kind:?} substituted operand {operand_position}"
            );
        }
        // A sibling carrier's bytes clamp to the wrong bounds (or not at
        // all): the decoded form must be rejected under every other kind.
        for (other_operation, other_carrier, other_kind, other_family) in saturating_kinds() {
            if (other_operation, other_carrier) == (operation, carrier) {
                continue;
            }
            let same_words = clang_words(other_operation, other_carrier) == expected;
            assert_eq!(
                validate_aarch64_selected_form_encoding(
                    &physical,
                    other_kind,
                    alternative(other_family),
                    &operands,
                    &expected,
                )
                .is_ok(),
                same_words,
                "{kind:?} bytes under {other_kind:?}"
            );
        }
    }
}

/// The realizations that share bytes are exactly the unsigned subtracts, the
/// unsigned divides, and every remainder: a zero-normalized narrow difference
/// borrows when the u64 one does, unsigned division never overflows, and the
/// divide/`msub` remainder pair is carrier-width independent within each
/// sign class.
#[test]
fn only_unsigned_subtract_divide_and_remainders_share_a_realization_across_carriers() {
    let mut shared = Vec::new();
    for (operation, carrier, _, _) in saturating_kinds() {
        let words = clang_words(operation, carrier);
        for (other_operation, other_carrier, _, _) in saturating_kinds() {
            if (other_operation, other_carrier) != (operation, carrier)
                && clang_words(other_operation, other_carrier) == words
            {
                shared.push((operation, carrier));
                break;
            }
        }
    }
    assert!(shared.iter().all(|(operation, carrier)| {
        (!carrier.is_signed()
            && matches!(
                operation,
                SaturatingOperation::Subtract | SaturatingOperation::Divide
            ))
            || *operation == SaturatingOperation::Remainder
    }));
    assert_eq!(shared.len(), 16);
}

#[test]
fn four_operand_saturating_forms_reject_aliased_outputs() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for (operation, carrier, kind, family) in saturating_kinds() {
        let key = alternative(family);
        if SaturatingRealization::of(operation, carrier).operand_count() != 4 {
            continue;
        }
        // Both outputs are early-clobber: neither the result nor the bound
        // scratch may alias an input, and they may not alias each other.
        for names in [
            ["x9", "x10", "x11", "x9"],
            ["x9", "x10", "x11", "x10"],
            ["x9", "x10", "x11", "x11"],
            ["x9", "x10", "x9", "x12"],
            ["x9", "x10", "x10", "x12"],
        ] {
            let aliased = names.map(|name| physical.model().view_named(name).unwrap().id);
            assert!(
                encode_aarch64_selected_form(&physical, kind, key, &aliased).is_err(),
                "{kind:?} {names:?}"
            );
        }
    }
}

/// The two's-complement reference: clamp the exact result to the carrier.
fn reference(
    operation: SaturatingOperation,
    carrier: SaturatingCarrier,
    left: i128,
    right: i128,
) -> i128 {
    let exact = match operation {
        SaturatingOperation::Add => left + right,
        SaturatingOperation::Subtract => left - right,
        SaturatingOperation::Divide => left / right,
        SaturatingOperation::Remainder => left % right,
        // Two u64 maxima overflow i128; any such product exceeds u64::MAX.
        SaturatingOperation::Multiply => left.checked_mul(right).unwrap_or(i128::MAX),
    };
    let (minimum, maximum) = carrier_bounds(carrier);
    exact.clamp(minimum, maximum)
}

fn carrier_bounds(carrier: SaturatingCarrier) -> (i128, i128) {
    if carrier.is_signed() {
        (
            i128::from(carrier.minimum_bits() as i64),
            i128::from(carrier.maximum_bits() as i64),
        )
    } else {
        (0, i128::from(carrier.maximum_bits()))
    }
}

/// Interpret the decoded words with AArch64 flag semantics; returns x3.
fn interpret(decoded: &[DecodedWord], left: i128, right: i128) -> i128 {
    let mut registers = [0_u64; 32];
    registers[1] = left as u64;
    registers[2] = right as u64;
    let (mut n, mut z, mut c, mut v) = (false, false, false, false);
    let read = |registers: &[u64; 32], index: u8| {
        if index == 31 {
            0
        } else {
            registers[index as usize]
        }
    };
    let set_flags = |n: &mut bool,
                     z: &mut bool,
                     c: &mut bool,
                     v: &mut bool,
                     left: u64,
                     right: u64,
                     subtract: bool|
     -> u64 {
        let (result, carry) = if subtract {
            let (result, borrow) = left.overflowing_sub(right);
            (result, !borrow)
        } else {
            left.overflowing_add(right)
        };
        let overflow = if subtract {
            (left as i64).overflowing_sub(right as i64).1
        } else {
            (left as i64).overflowing_add(right as i64).1
        };
        *n = (result as i64) < 0;
        *z = result == 0;
        *c = carry;
        *v = overflow;
        result
    };
    for word in decoded {
        match *word {
            DecodedWord::Add {
                left,
                right,
                destination,
            } => {
                registers[destination as usize] =
                    read(&registers, left).wrapping_add(read(&registers, right));
            }
            DecodedWord::Subtract {
                left,
                right,
                destination,
            } => {
                registers[destination as usize] =
                    read(&registers, left).wrapping_sub(read(&registers, right));
            }
            DecodedWord::SignedDivide {
                dividend,
                divisor,
                destination,
            } => {
                registers[destination as usize] = (read(&registers, dividend) as i64)
                    .wrapping_div(read(&registers, divisor) as i64)
                    as u64;
            }
            DecodedWord::UnsignedDivide {
                dividend,
                divisor,
                destination,
            } => {
                registers[destination as usize] =
                    read(&registers, dividend) / read(&registers, divisor);
            }
            DecodedWord::MultiplySubtract {
                left,
                right,
                minuend,
                destination,
            } => {
                registers[destination as usize] = read(&registers, minuend)
                    .wrapping_sub(read(&registers, left).wrapping_mul(read(&registers, right)));
            }
            DecodedWord::AddWithFlags {
                left,
                right,
                destination,
            } => {
                registers[destination as usize] = set_flags(
                    &mut n,
                    &mut z,
                    &mut c,
                    &mut v,
                    read(&registers, left),
                    read(&registers, right),
                    false,
                );
            }
            DecodedWord::SubtractWithFlags {
                left,
                right,
                destination,
            } => {
                registers[destination as usize] = set_flags(
                    &mut n,
                    &mut z,
                    &mut c,
                    &mut v,
                    read(&registers, left),
                    read(&registers, right),
                    true,
                );
            }
            DecodedWord::Compare { left, right } => {
                set_flags(
                    &mut n,
                    &mut z,
                    &mut c,
                    &mut v,
                    read(&registers, left),
                    read(&registers, right),
                    true,
                );
            }
            DecodedWord::SelectMaximumOnCarry {
                source,
                destination,
            } => {
                // csinv destination, source, xzr, cc
                registers[destination as usize] = if c {
                    u64::MAX
                } else {
                    read(&registers, source)
                };
            }
            DecodedWord::SelectZeroOnBorrow {
                source,
                destination,
            } => {
                // csel destination, source, xzr, cs
                registers[destination as usize] = if c { read(&registers, source) } else { 0 };
            }
            DecodedWord::MaterializeBound { destination, value } => {
                registers[destination as usize] = value;
            }
            DecodedWord::SelectOnGreater {
                source,
                destination,
            } => {
                if !z && n == v {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::SelectOnLess {
                source,
                destination,
            } => {
                if n != v {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::SelectOnEqual {
                source,
                destination,
            } => {
                if z {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::SelectOnOverflow {
                source,
                destination,
            } => {
                if v {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::ArithmeticShiftRight63 {
                source,
                destination,
            } => {
                registers[destination as usize] = ((read(&registers, source) as i64) >> 63) as u64;
            }
            DecodedWord::ExclusiveOrI64Maximum { register } => {
                registers[register as usize] ^= i64::MAX as u64;
            }
            DecodedWord::Multiply {
                left,
                right,
                destination,
            } => {
                registers[destination as usize] =
                    read(&registers, left).wrapping_mul(read(&registers, right));
            }
            DecodedWord::SignedMultiplyHigh {
                left,
                right,
                destination,
            } => {
                let product = i128::from(read(&registers, left) as i64)
                    * i128::from(read(&registers, right) as i64);
                registers[destination as usize] = (product >> 64) as u64;
            }
            DecodedWord::UnsignedMultiplyHigh {
                left,
                right,
                destination,
            } => {
                let product =
                    u128::from(read(&registers, left)) * u128::from(read(&registers, right));
                registers[destination as usize] = (product >> 64) as u64;
            }
            DecodedWord::CompareWithSignOf { high, low } => {
                let sign = ((read(&registers, low) as i64) >> 63) as u64;
                set_flags(
                    &mut n,
                    &mut z,
                    &mut c,
                    &mut v,
                    read(&registers, high),
                    sign,
                    true,
                );
            }
            DecodedWord::CompareZero { source } => {
                set_flags(
                    &mut n,
                    &mut z,
                    &mut c,
                    &mut v,
                    read(&registers, source),
                    0,
                    true,
                );
            }
            DecodedWord::SelectOnNotEqual {
                source,
                destination,
            } => {
                if !z {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::SelectOnHigher {
                source,
                destination,
            } => {
                if c && !z {
                    registers[destination as usize] = read(&registers, source);
                }
            }
            DecodedWord::SelectMaximumUnlessEqual {
                source,
                destination,
            } => {
                // csinv destination, source, xzr, eq
                registers[destination as usize] = if z {
                    read(&registers, source)
                } else {
                    u64::MAX
                };
            }
            DecodedWord::ConditionalCompareMinusOne { register } => {
                if z {
                    set_flags(
                        &mut n,
                        &mut z,
                        &mut c,
                        &mut v,
                        read(&registers, register),
                        1,
                        false,
                    );
                } else {
                    (n, z, c, v) = (false, false, false, false);
                }
            }
            other => panic!("unexpected saturating word {other:?}"),
        }
    }
    assert_eq!((registers[1], registers[2]), (left as u64, right as u64));
    i128::from(registers[3] as i64)
}

#[test]
fn every_saturating_carrier_clamps_its_edges_in_the_decoded_words() {
    let physical = validate_physical_register_model(aarch64_physical_register_model()).unwrap();
    for (operation, carrier, kind, family) in saturating_kinds() {
        let operand_count = SaturatingRealization::of(operation, carrier).operand_count();
        let operands: Vec<_> = ["x1", "x2", "x3", "x4"][..operand_count]
            .iter()
            .map(|name| physical.model().view_named(name).unwrap().id)
            .collect();
        let encoded =
            encode_aarch64_selected_form(&physical, kind, alternative(family), &operands).unwrap();
        let decoded = decode_words(encoded.bytes()).unwrap();
        let (minimum, maximum) = carrier_bounds(carrier);
        let mut samples = vec![minimum, minimum + 1, 0, 1, 2, 7, 40, maximum - 1, maximum];
        if carrier.is_signed() {
            samples.extend([-1, -2, -7, -40]);
        }
        for &left in &samples {
            for &right in &samples {
                if matches!(
                    operation,
                    SaturatingOperation::Divide | SaturatingOperation::Remainder
                ) && right == 0
                {
                    continue;
                }
                // Registers hold the carrier's normalized 64-bit pattern;
                // `interpret` returns x3 as a signed i64, so an unsigned
                // carrier reads its pattern back through u64.
                let observed = interpret(&decoded, left, right);
                let observed = if carrier.is_signed() {
                    observed
                } else {
                    i128::from(observed as u64)
                };
                assert_eq!(
                    observed,
                    reference(operation, carrier, left, right),
                    "{kind:?} {left} {right}"
                );
            }
        }
    }
}
