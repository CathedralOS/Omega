//! SSE2 unsigned-64 conversion and destination-owned conversion guards.
//!
//! Every path consumes the input before reusing the result register as scratch.
//! Only xmm4/xmm5 and the declared result are written. Local branches change
//! RFLAGS, but never leave the selected form except at its explicit Crash.

use super::{IeeeFloatIntegerCarrier, Instruction, LEFT_SCRATCH, RIGHT_SCRATCH};

const ZERO: u8 = 4;
const NOT_ZERO: u8 = 5;
const NOT_SIGNED: u8 = 9;
const LESS: u8 = 1;
const LESS_EQUAL: u8 = 2;
const TWO_TO_64: u64 = 0x43f0_0000_0000_0000;

fn bound(result: u8, bits: u64) -> [Instruction; 2] {
    [
        Instruction::MoveImmediate64 {
            general: result,
            value: bits,
        },
        Instruction::Transfer {
            to_float: true,
            wide: true,
            xmm: RIGHT_SCRATCH,
            general: result,
        },
    ]
}

fn observe_mask(result: u8) -> [Instruction; 2] {
    [
        Instruction::Transfer {
            to_float: false,
            wide: true,
            xmm: RIGHT_SCRATCH,
            general: result,
        },
        Instruction::Test64 { general: result },
    ]
}

fn scalar(opcode: u8) -> Instruction {
    Instruction::Scalar {
        double: true,
        opcode,
        destination: LEFT_SCRATCH,
        source: RIGHT_SCRATCH,
    }
}

fn compare_bound(predicate: u8) -> Instruction {
    Instruction::CompareScalar {
        double: true,
        predicate,
        destination: RIGHT_SCRATCH,
        source: LEFT_SCRATCH,
    }
}

fn encoded_length(instructions: &[Instruction]) -> i32 {
    let mut bytes = Vec::new();
    for instruction in instructions {
        super::encode_instruction(&mut bytes, *instruction);
    }
    // These fixed conversion arms contain fewer than 256 bytes, independent
    // of input values. Branch offsets are not supplied by an artifact.
    bytes.len() as i32
}

fn choose(
    program: &mut Vec<Instruction>,
    condition: u8,
    taken: Vec<Instruction>,
    not_taken: Vec<Instruction>,
) {
    program.push(Instruction::ConditionalJump {
        condition,
        displacement: encoded_length(&not_taken) + 5,
    });
    program.extend(not_taken);
    program.push(Instruction::Jump {
        displacement: encoded_length(&taken),
    });
    program.extend(taken);
}

/// Above the signed range, `(value >> 1) | (value & 1)` retains every bit
/// relevant to rounding, including the sticky remainder. Converting this
/// signed half directly to the requested format and then doubling exactly
/// produces one nearest-even rounding of the original unsigned integer.
pub(super) fn from_unsigned64(double: bool, source: u8, result: u8) -> Vec<Instruction> {
    let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
    let mut program = vec![
        Instruction::Transfer {
            to_float: true,
            wide: true,
            xmm: left,
            general: source,
        },
        Instruction::Test64 { general: source },
    ];
    let low = vec![Instruction::IntegerToFloat {
        double,
        wide: true,
        xmm: left,
        general: source,
    }];
    let high = vec![
        Instruction::MoveAligned {
            destination: right,
            source: left,
        },
        Instruction::ShiftLeft {
            quad: true,
            xmm: right,
            count: 63,
        },
        Instruction::ShiftRight {
            quad: true,
            arithmetic: false,
            xmm: right,
            count: 63,
        },
        Instruction::ShiftRight {
            quad: true,
            arithmetic: false,
            xmm: left,
            count: 1,
        },
        Instruction::Or {
            destination: left,
            source: right,
        },
        Instruction::Transfer {
            to_float: false,
            wide: true,
            xmm: left,
            general: result,
        },
        Instruction::IntegerToFloat {
            double,
            wide: true,
            xmm: left,
            general: result,
        },
        Instruction::Scalar {
            double,
            opcode: 0x58,
            destination: left,
            source: left,
        },
    ];
    choose(&mut program, NOT_SIGNED, low, high);
    program.push(Instruction::Transfer {
        to_float: false,
        wide: double,
        xmm: left,
        general: result,
    });
    program
}

/// xmm4 already contains the exactly widened input. Clamp NaN and negative
/// values to zero; values at or above 2^64 select u64::MAX before conversion.
/// In [2^63, 2^64), subtracting 2^63 is exact, the signed conversion fits,
/// and restoring bit 63 reconstructs the unsigned truncated result.
pub(super) fn to_unsigned64(result: u8) -> Vec<Instruction> {
    let (left, right) = (LEFT_SCRATCH, RIGHT_SCRATCH);
    let mut program = vec![
        Instruction::MoveAligned {
            destination: right,
            source: left,
        },
        Instruction::CompareScalar {
            double: true,
            predicate: super::ORDERED,
            destination: right,
            source: left,
        },
        Instruction::And {
            destination: left,
            source: right,
        },
        Instruction::Xor {
            double: true,
            destination: right,
            source: right,
        },
        scalar(0x5f),
    ];
    program.extend(bound(result, TWO_TO_64));
    program.push(compare_bound(LESS_EQUAL));
    program.extend(observe_mask(result));

    let mut in_range = Vec::new();
    in_range.extend(bound(result, super::TWO_TO_63));
    in_range.push(compare_bound(LESS_EQUAL));
    in_range.extend(observe_mask(result));
    let low = vec![Instruction::FloatToInteger {
        wide: true,
        general: result,
        xmm: left,
    }];
    let mut high = Vec::from(bound(result, super::TWO_TO_63));
    high.extend([
        scalar(0x5c),
        Instruction::FloatToInteger {
            wide: true,
            general: result,
            xmm: left,
        },
        Instruction::Transfer {
            to_float: true,
            wide: true,
            xmm: left,
            general: result,
        },
        Instruction::AllOnes { xmm: right },
        Instruction::ShiftLeft {
            quad: true,
            xmm: right,
            count: 63,
        },
        Instruction::Or {
            destination: left,
            source: right,
        },
        Instruction::Transfer {
            to_float: false,
            wide: true,
            xmm: left,
            general: result,
        },
    ]);
    choose(&mut in_range, NOT_ZERO, high, low);
    choose(
        &mut program,
        NOT_ZERO,
        vec![Instruction::MoveImmediate64 {
            general: result,
            value: u64::MAX,
        }],
        in_range,
    );
    program
}

/// The lower open endpoint is MIN-1, not MIN: truncation of a negative
/// fractional value may still produce MIN. Binary64 cannot represent i64's
/// MIN-1, so only that carrier uses the inclusive MIN comparison instead.
fn interval(target: IeeeFloatIntegerCarrier) -> (u64, u8, u64) {
    use IeeeFloatIntegerCarrier::*;
    match target {
        I8 => (0xc060_2000_0000_0000, LESS, 0x4060_0000_0000_0000),
        I16 => (0xc0e0_0020_0000_0000, LESS, 0x40e0_0000_0000_0000),
        I32 => (0xc1e0_0000_0020_0000, LESS, super::TWO_TO_31),
        I64 => (0xc3e0_0000_0000_0000, LESS_EQUAL, super::TWO_TO_63),
        U8 => (0xbff0_0000_0000_0000, LESS, 0x4070_0000_0000_0000),
        U16 => (0xbff0_0000_0000_0000, LESS, 0x40f0_0000_0000_0000),
        U32 => (0xbff0_0000_0000_0000, LESS, 0x41f0_0000_0000_0000),
        U64 => (0xbff0_0000_0000_0000, LESS, TWO_TO_64),
    }
}

pub(super) fn trapping(
    double: bool,
    target: IeeeFloatIntegerCarrier,
    source: u8,
    result: u8,
) -> Vec<Instruction> {
    let mut program = vec![Instruction::Transfer {
        to_float: true,
        wide: double,
        xmm: LEFT_SCRATCH,
        general: source,
    }];
    if !double {
        program.push(Instruction::Scalar {
            double: false,
            opcode: 0x5a,
            destination: LEFT_SCRATCH,
            source: LEFT_SCRATCH,
        });
    }
    let (minimum, lower_predicate, upper) = interval(target);
    program.extend(bound(result, minimum));
    program.push(compare_bound(lower_predicate));
    program.extend(observe_mask(result));
    // Both lower comparisons are ordered: NaN cannot branch over this Crash.
    program.extend([
        Instruction::ConditionalJump {
            condition: NOT_ZERO,
            displacement: 2,
        },
        Instruction::Crash,
    ]);
    program.extend(bound(result, upper));
    program.push(compare_bound(LESS_EQUAL));
    program.extend(observe_mask(result));
    program.extend([
        Instruction::ConditionalJump {
            condition: ZERO,
            displacement: 2,
        },
        Instruction::Crash,
    ]);
    program.extend(super::truncation_landed(target, result));
    program
}

#[cfg(test)]
mod tests;
