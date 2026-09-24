//! How each Trapping form is realized on AArch64.
//!
//! A Trapping form is one selected instruction: it computes the exact result
//! and branches over an inline `brk #0` (the `Crash` leaf's word, so the
//! process stops through the same `SIGTRAP` route) exactly when the settled
//! Trapping predicate holds. The word before every `brk` is the only branch in
//! the form, a `b.<no-trap> #8` or `cbnz #8` that skips the trap, so control
//! either falls through to the next selected instruction or stops in place.
//!
//! Scalar transport keeps narrow (8/16/32-bit) carriers sign- or
//! zero-normalized in 64-bit registers, so the 64-bit sum, difference,
//! product, quotient or left shift of normalized narrow operands is exact,
//! and the result lies in the carrier exactly when it equals the extension of
//! its own low carrier bits: `cmp x, w, sxt*|uxt*` then `b.eq` decides every
//! narrow range check in one word. The 64-bit carriers have no headroom, so
//! they read the overflow (`vs`) or carry (`cs`/`cc`) flag, the `smulh`/
//! `umulh` high half, or shift back and compare. Division checks its zero
//! divisor with `cbnz` and, for i64, the one wrapping quotient MIN / -1 with
//! `cmn divisor, #1; ccmp dividend, #1, #0, eq`, which leaves V set exactly
//! when the divisor is -1 and the dividend is MIN; narrow signed quotients
//! are range-checked after `sdiv`, which on normalized narrow operands
//! cannot wrap. `sdiv`/`udiv` never fault on AArch64, so every trap here is
//! explicit. A remainder traps on the same predicate as its quotient (MIN %
//! -1 has an unrepresentable common quotient), then `msub` recovers it.
//!
//! A shift count of any fixed type is normalized in its register, so an
//! unsigned `cmp count, #width; b.lo` admits exactly the counts inside
//! `0..width` (a negative signed count reads as a huge unsigned value). A
//! conversion needs no arithmetic: a signed source into a narrower signed
//! carrier uses the extension compare, and every other non-trivial pair asks
//! whether any bit at or above the destination's value bits is set with one
//! `tst` bitmask immediate. Every form declares NZCV clobbered.
use super::decoding::DecodedWord;
use register_model::RegisterConstraintKey;
use selected_instructions::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedTrapBehavior, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, SaturatingCarrier, TrappingForm, TrappingOperation,
};
use semantic_vocabulary::IntegerSign;

/// The `b.eq`, `b.hs`, `b.lo` and `b.vc` conditions a trap skip branches on:
/// each holds exactly when the checked predicate is false.
const EQUAL: u8 = 0;
const CARRY_SET: u8 = 2;
const CARRY_CLEAR: u8 = 3;
const OVERFLOW_CLEAR: u8 = 7;

/// The `cmp x, w, <extension>` option naming a narrow carrier's extension.
const fn extension(carrier: SaturatingCarrier) -> u8 {
    match carrier {
        SaturatingCarrier::U8 => 0,
        SaturatingCarrier::U16 => 1,
        SaturatingCarrier::U32 => 2,
        SaturatingCarrier::I8 => 4,
        SaturatingCarrier::I16 => 5,
        SaturatingCarrier::I32 => 6,
        SaturatingCarrier::I64 | SaturatingCarrier::U64 => {
            panic!("64-bit carriers have no narrow extension check")
        }
    }
}

/// The encoded effects every Trapping form shares: its operand reads and
/// writes, NZCV clobbered, and control that falls through or stops at the
/// inline trap.
pub(crate) fn effects(reads: Vec<u16>, writes: Vec<u16>) -> MachineEncodedEffects {
    let physical = crate::aarch64_physical_register_model();
    MachineEncodedEffects {
        implicit_unit_clobbers: physical
            .view_named("nzcv")
            .expect("canonical AArch64 model declares nzcv")
            .units
            .clone(),
        trap: MachineEncodedTrapBehavior::TrappingIntegerV1,
        control: MachineEncodedControlEffect::FallThroughOrTrapV1,
        ..MachineEncodedEffects::fallthrough_v1(reads, writes)
    }
}

/// The catalog row of one Trapping form: an external-effect barrier (the
/// trap may stop the process in place, so the form never moves across a
/// hosted effect and is never dead), no memory, and one exact-size
/// alternative.
pub(crate) fn declaration(
    form: TrappingForm,
    constraint: RegisterConstraintKey,
) -> MachineEffectDeclaration {
    let sources = form.source_count() as u16;
    MachineEffectDeclaration {
        semantic: MachineSemanticKind::TrappingInteger(form),
        constraint,
        memory: MachineMemoryEffect::NoneV1,
        trap: MachineTrapBehavior::TrappingIntegerV1,
        barrier: MachineBarrier::ExternalEffect,
        call: MachineCallEffect::NoneV1,
        cleanup: MachineCleanupEffect::NoneV1,
        alternatives: vec![MachineAlternative {
            key: MachineAlternativeKey {
                family: MachineAlternativeFamily::TrappingInteger(form),
                variant: 0,
            },
            applicability: MachineAlternativeApplicability::Always,
            size: MachineSizeKnowledge::ExactBytes(byte_size(form)),
            latency: MachineLatencyKnowledge::StableBaselineUnavailable,
            encoded: effects(
                (0..sources).collect(),
                (sources..operand_count(form) as u16).collect(),
            ),
        }],
    }
}

/// `[left, right, result, scratch]` for binary arithmetic and shifts,
/// `[operand, result, scratch]` for a conversion.
pub(crate) const fn operand_count(form: TrappingForm) -> usize {
    form.source_count() + 2
}

/// The low bit from which a set bit makes the conversion unrepresentable,
/// or `None` when every source value fits (a signed source into i64, an
/// unsigned source into u64) or the signed-extension compare decides it.
const fn conversion_high_bit(source: IntegerSign, carrier: SaturatingCarrier) -> Option<u8> {
    match (source, carrier) {
        (IntegerSign::Signed, SaturatingCarrier::I64)
        | (IntegerSign::Unsigned, SaturatingCarrier::U64) => None,
        (IntegerSign::Signed, carrier) if carrier.is_signed() => None,
        // A negative signed source has its sign bit set; a non-negative one
        // fits u64 exactly.
        (IntegerSign::Signed, SaturatingCarrier::U64) => Some(63),
        (_, carrier) if carrier.is_signed() => Some(carrier.bits() as u8 - 1),
        (_, carrier) => Some(carrier.bits() as u8),
    }
}

/// The semantic word sequence realizing `form` over `registers`. The encoder
/// emits exactly these words, and the decoder check requires the decoded
/// bytes to equal them.
pub(crate) fn realization(form: TrappingForm, registers: &[u8]) -> Vec<DecodedWord> {
    let carrier = form.carrier;
    let skip = |condition: u8| DecodedWord::BranchOverTrap { condition };
    let narrow_check = |value: u8| {
        [
            DecodedWord::CompareExtended {
                left: value,
                right: value,
                extension: extension(carrier),
            },
            skip(EQUAL),
            DecodedWord::Crash,
        ]
    };
    let mut words = Vec::new();
    if let TrappingOperation::Convert { source } = form.operation {
        let (operand, result) = (registers[0], registers[1]);
        match (source, conversion_high_bit(source, carrier)) {
            (_, Some(bit)) => words.extend([
                DecodedWord::TestHighBits {
                    source: operand,
                    from_bit: bit,
                },
                skip(EQUAL),
                DecodedWord::Crash,
            ]),
            (IntegerSign::Signed, None) if carrier != SaturatingCarrier::I64 => {
                words.extend(narrow_check(operand));
            }
            (_, None) => {}
        }
        words.push(DecodedWord::Copy {
            source: operand,
            destination: result,
        });
        return words;
    }
    let (left, right, value, scratch) = (registers[0], registers[1], registers[2], registers[3]);
    match form.operation {
        TrappingOperation::Add | TrappingOperation::Subtract if !carrier.is_narrow() => {
            let destination = value;
            words.push(if form.operation == TrappingOperation::Add {
                DecodedWord::AddWithFlags {
                    left,
                    right,
                    destination,
                }
            } else {
                DecodedWord::SubtractWithFlags {
                    left,
                    right,
                    destination,
                }
            });
            words.push(skip(match (form.operation, carrier.is_signed()) {
                (_, true) => OVERFLOW_CLEAR,
                (TrappingOperation::Add, false) => CARRY_CLEAR,
                _ => CARRY_SET,
            }));
            words.push(DecodedWord::Crash);
        }
        TrappingOperation::Add | TrappingOperation::Subtract | TrappingOperation::Multiply
            if carrier.is_narrow() =>
        {
            let destination = value;
            words.push(match form.operation {
                TrappingOperation::Add => DecodedWord::Add {
                    left,
                    right,
                    destination,
                },
                TrappingOperation::Subtract => DecodedWord::Subtract {
                    left,
                    right,
                    destination,
                },
                _ => DecodedWord::Multiply {
                    left,
                    right,
                    destination,
                },
            });
            words.extend(narrow_check(value));
        }
        TrappingOperation::Multiply => {
            words.push(if carrier.is_signed() {
                DecodedWord::SignedMultiplyHigh {
                    left,
                    right,
                    destination: scratch,
                }
            } else {
                DecodedWord::UnsignedMultiplyHigh {
                    left,
                    right,
                    destination: scratch,
                }
            });
            words.push(DecodedWord::Multiply {
                left,
                right,
                destination: value,
            });
            // The signed product fits when the high half is the low half's
            // sign; the unsigned one when the high half is zero.
            words.push(if carrier.is_signed() {
                DecodedWord::CompareWithSignOf {
                    high: scratch,
                    low: value,
                }
            } else {
                DecodedWord::CompareZero { source: scratch }
            });
            words.push(skip(EQUAL));
            words.push(DecodedWord::Crash);
        }
        TrappingOperation::Divide | TrappingOperation::Remainder => {
            words.push(DecodedWord::BranchNonZeroOverTrap { register: right });
            words.push(DecodedWord::Crash);
            if carrier == SaturatingCarrier::I64 {
                words.push(DecodedWord::CompareNegativeOne { source: right });
                words.push(DecodedWord::ConditionalCompareOneOnEqual { register: left });
                words.push(skip(OVERFLOW_CLEAR));
                words.push(DecodedWord::Crash);
            }
            // The divide writes the result directly, or the scratch when
            // `msub` still needs the original operands.
            let quotient = if form.operation == TrappingOperation::Divide {
                value
            } else {
                scratch
            };
            words.push(if carrier.is_signed() {
                DecodedWord::SignedDivide {
                    dividend: left,
                    divisor: right,
                    destination: quotient,
                }
            } else {
                DecodedWord::UnsignedDivide {
                    dividend: left,
                    divisor: right,
                    destination: quotient,
                }
            });
            if carrier.is_signed() && carrier.is_narrow() {
                // The only out-of-carrier narrow quotient is MIN / -1.
                words.extend(narrow_check(quotient));
            }
            if form.operation == TrappingOperation::Remainder {
                words.push(DecodedWord::MultiplySubtract {
                    left: scratch,
                    right,
                    minuend: left,
                    destination: value,
                });
            }
        }
        TrappingOperation::ShiftLeft | TrappingOperation::ShiftRight => {
            let (shifted, count) = (left, right);
            words.push(DecodedWord::CompareImmediate {
                source: count,
                immediate: carrier.bits(),
            });
            words.push(skip(CARRY_CLEAR));
            words.push(DecodedWord::Crash);
            if form.operation == TrappingOperation::ShiftRight {
                words.push(if carrier.is_signed() {
                    DecodedWord::ShiftRightArithmeticVariable {
                        value: shifted,
                        count,
                        destination: value,
                    }
                } else {
                    DecodedWord::ShiftRightLogicalVariable {
                        value: shifted,
                        count,
                        destination: value,
                    }
                });
                return words;
            }
            words.push(DecodedWord::ShiftLeftVariable {
                value: shifted,
                count,
                destination: value,
            });
            if carrier.is_narrow() {
                // A count below 32 keeps a normalized narrow value's left
                // shift exact in 64 bits.
                words.extend(narrow_check(value));
            } else {
                // Shifting back recovers the value exactly when no
                // significant bit was lost.
                words.push(if carrier.is_signed() {
                    DecodedWord::ShiftRightArithmeticVariable {
                        value,
                        count,
                        destination: scratch,
                    }
                } else {
                    DecodedWord::ShiftRightLogicalVariable {
                        value,
                        count,
                        destination: scratch,
                    }
                });
                words.push(DecodedWord::Compare {
                    left: scratch,
                    right: shifted,
                });
                words.push(skip(EQUAL));
                words.push(DecodedWord::Crash);
            }
        }
        TrappingOperation::Add | TrappingOperation::Subtract => {
            unreachable!("narrow and 64-bit add/subtract are both matched above")
        }
        TrappingOperation::Convert { .. } => unreachable!("conversions return above"),
    }
    words
}

/// The exact byte size of every realization; register choice never changes
/// an AArch64 word count.
pub(crate) fn byte_size(form: TrappingForm) -> u16 {
    let registers: Vec<u8> = (1..=operand_count(form) as u8).collect();
    4 * realization(form, &registers).len() as u16
}

/// The machine word of one realization step.
pub(crate) fn encode_word(word: DecodedWord) -> u32 {
    let three = |opcode: u32, left: u8, right: u8, destination: u8| {
        opcode | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination)
    };
    match word {
        DecodedWord::Crash => 0xd420_0000,
        DecodedWord::BranchOverTrap { condition } => 0x5400_0040 | u32::from(condition),
        DecodedWord::BranchNonZeroOverTrap { register } => 0xb500_0040 | u32::from(register),
        DecodedWord::CompareExtended {
            left,
            right,
            extension,
        } => {
            0xeb20_001f
                | (u32::from(right) << 16)
                | (u32::from(extension) << 13)
                | (u32::from(left) << 5)
        }
        DecodedWord::CompareNegativeOne { source } => 0xb100_041f | (u32::from(source) << 5),
        DecodedWord::ConditionalCompareOneOnEqual { register } => {
            0xfa41_0800 | (u32::from(register) << 5)
        }
        DecodedWord::TestHighBits { source, from_bit } => {
            0xf240_001f
                | ((64 - u32::from(from_bit)) << 16)
                | ((63 - u32::from(from_bit)) << 10)
                | (u32::from(source) << 5)
        }
        DecodedWord::AddWithFlags {
            left,
            right,
            destination,
        } => three(0xab00_0000, left, right, destination),
        DecodedWord::SubtractWithFlags {
            left,
            right,
            destination,
        } => three(0xeb00_0000, left, right, destination),
        DecodedWord::Add {
            left,
            right,
            destination,
        } => three(0x8b00_0000, left, right, destination),
        DecodedWord::Subtract {
            left,
            right,
            destination,
        } => three(0xcb00_0000, left, right, destination),
        DecodedWord::Multiply {
            left,
            right,
            destination,
        } => three(0x9b00_7c00, left, right, destination),
        DecodedWord::SignedMultiplyHigh {
            left,
            right,
            destination,
        } => three(0x9b40_7c00, left, right, destination),
        DecodedWord::UnsignedMultiplyHigh {
            left,
            right,
            destination,
        } => three(0x9bc0_7c00, left, right, destination),
        DecodedWord::CompareWithSignOf { high, low } => {
            0xeb80_fc1f | (u32::from(low) << 16) | (u32::from(high) << 5)
        }
        DecodedWord::CompareZero { source } => 0xf100_001f | (u32::from(source) << 5),
        DecodedWord::CompareImmediate { source, immediate } => {
            0xf100_001f | (u32::from(immediate) << 10) | (u32::from(source) << 5)
        }
        DecodedWord::Compare { left, right } => {
            0xeb00_001f | (u32::from(right) << 16) | (u32::from(left) << 5)
        }
        DecodedWord::SignedDivide {
            dividend,
            divisor,
            destination,
        } => three(0x9ac0_0c00, dividend, divisor, destination),
        DecodedWord::UnsignedDivide {
            dividend,
            divisor,
            destination,
        } => three(0x9ac0_0800, dividend, divisor, destination),
        DecodedWord::MultiplySubtract {
            left,
            right,
            minuend,
            destination,
        } => {
            0x9b00_8000
                | (u32::from(right) << 16)
                | (u32::from(minuend) << 10)
                | (u32::from(left) << 5)
                | u32::from(destination)
        }
        DecodedWord::ShiftLeftVariable {
            value,
            count,
            destination,
        } => three(0x9ac0_2000, value, count, destination),
        DecodedWord::ShiftRightLogicalVariable {
            value,
            count,
            destination,
        } => three(0x9ac0_2400, value, count, destination),
        DecodedWord::ShiftRightArithmeticVariable {
            value,
            count,
            destination,
        } => three(0x9ac0_2800, value, count, destination),
        DecodedWord::Copy {
            source,
            destination,
        } => 0xaa00_03e0 | (u32::from(source) << 16) | u32::from(destination),
        other => unreachable!("{other:?} is not a Trapping realization word"),
    }
}

#[cfg(test)]
mod tests;
