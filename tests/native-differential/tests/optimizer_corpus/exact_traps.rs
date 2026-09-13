use sha2::{Digest, Sha256};

use super::generator::{GENERATOR, SEED, next};

pub(super) const FORMAT: &str = "omega.optimizer-corpus.exact-traps.v1";
pub(super) const CASE_COUNT: usize = 64;

/// Trap-capable integer leaves admitted under one discharged definedness
/// obligation each. Every leaf returns u64 so the host-native caller keeps one
/// ABI shape; the cast leaf narrows to u8 and widens back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrapOperation {
    ExactAdd,
    ExactSubtract,
    ExactDivide,
    NarrowingCast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TrapCase {
    pub(super) ordinal: usize,
    pub(super) operation: TrapOperation,
    pub(super) left: u64,
    pub(super) right: u64,
    pub(super) expected: u64,
}

pub(super) fn cases() -> Vec<TrapCase> {
    let mut state = SEED;
    let cases = (0..CASE_COUNT)
        .map(|ordinal| {
            // Low LCG bits cycle with period 2^k, so kind selectors read the high word.
            let operation = match (next(&mut state) >> 32) % 4 {
                0 => TrapOperation::ExactAdd,
                1 => TrapOperation::ExactSubtract,
                2 => TrapOperation::ExactDivide,
                _ => TrapOperation::NarrowingCast,
            };
            let (left, right, expected) = match operation {
                TrapOperation::ExactAdd => {
                    let left = next(&mut state);
                    let headroom = u64::MAX - left;
                    let right = match next(&mut state) >> 61 {
                        0..=3 => next(&mut state) & headroom,
                        4..=6 => headroom,
                        _ => headroom & 0xffff,
                    };
                    (
                        left,
                        right,
                        left.checked_add(right)
                            .expect("seeded exact-add operand must stay representable"),
                    )
                }
                TrapOperation::ExactSubtract => {
                    let left = next(&mut state);
                    let right = match next(&mut state) >> 61 {
                        0..=3 => left & next(&mut state),
                        4..=6 => left,
                        _ => 0,
                    };
                    (
                        left,
                        right,
                        left.checked_sub(right)
                            .expect("seeded exact-subtract operand must stay representable"),
                    )
                }
                TrapOperation::ExactDivide => {
                    let left = next(&mut state);
                    let right = match next(&mut state) >> 61 {
                        0..=3 => (next(&mut state) & 0xffff_ffff) | 1,
                        4..=5 => 1,
                        6 => u64::MAX,
                        _ => next(&mut state) | 1,
                    };
                    (left, right, left / right)
                }
                TrapOperation::NarrowingCast => {
                    let left = match next(&mut state) >> 61 {
                        0..=3 => next(&mut state) & 0xff,
                        4 => 0,
                        5 => 0xff,
                        _ => next(&mut state) & 0x7f,
                    };
                    (left, 0, left)
                }
            };
            TrapCase {
                ordinal,
                operation,
                left,
                right,
                expected,
            }
        })
        .collect::<Vec<_>>();
    assert_coverage(&cases);
    cases
}

pub(super) fn validate_manifest(cases: &[TrapCase]) {
    assert_eq!(cases.len(), CASE_COUNT);
    let rendered = format!(
        "format={FORMAT}\ngenerator={GENERATOR}\nseed={SEED:#018x}\ncase_count={CASE_COUNT}\npsi_shape=boolean_conditional_identical_exact_trap_leaves\nhost_native_shape=boolean_conditional_identical_exact_trap_leaves_same_artifact\nhost_native_oracle=terminal_interpreter_equals_postallocation_optimized_native_u64\noperand_lane=exact_add_subtract_divide_u64_and_u64_to_u8_cast_widen\nrecords_sha256={}\n",
        records_digest(cases),
    );
    let checked_in = include_str!("../../corpora/optimizer/v2/conditional_exact_trap_lanes.txt");
    assert_eq!(
        checked_in, rendered,
        "trap optimizer corpus manifest drifted"
    );
}

fn records_digest(cases: &[TrapCase]) -> String {
    let mut hasher = Sha256::new();
    for case in cases {
        hasher.update((case.ordinal as u64).to_le_bytes());
        hasher.update(operation_code(case.operation).to_le_bytes());
        hasher.update(case.left.to_le_bytes());
        hasher.update(case.right.to_le_bytes());
        hasher.update(case.expected.to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn operation_code(operation: TrapOperation) -> u8 {
    match operation {
        TrapOperation::ExactAdd => 0,
        TrapOperation::ExactSubtract => 1,
        TrapOperation::ExactDivide => 2,
        TrapOperation::NarrowingCast => 3,
    }
}

fn assert_coverage(cases: &[TrapCase]) {
    for operation in [
        TrapOperation::ExactAdd,
        TrapOperation::ExactSubtract,
        TrapOperation::ExactDivide,
        TrapOperation::NarrowingCast,
    ] {
        assert!(cases.iter().any(|case| case.operation == operation));
    }
    assert!(cases.iter().any(|case| {
        case.operation == TrapOperation::ExactAdd && case.left + case.right == u64::MAX
    }));
    assert!(
        cases
            .iter()
            .any(|case| { case.operation == TrapOperation::ExactSubtract && case.expected == 0 })
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.operation == TrapOperation::ExactSubtract && case.right == 0 })
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.operation == TrapOperation::ExactDivide && case.right == 1 })
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.operation == TrapOperation::ExactDivide && case.right == u64::MAX })
    );
    assert!(
        cases
            .iter()
            .any(|case| { case.operation == TrapOperation::NarrowingCast && case.left == 0 })
    );
    assert!(cases.iter().any(|case| {
        case.operation == TrapOperation::NarrowingCast && case.left == u64::from(u8::MAX)
    }));
}
